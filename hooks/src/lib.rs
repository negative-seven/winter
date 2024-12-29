#![allow(clippy::missing_panics_doc)]

mod hooks;
mod state;

use futures::executor::block_on;
pub use shared::ipc::message::LogLevel;
use shared::{
    input::MouseButton,
    ipc::{
        message::{self, Message},
        Sender,
    },
    windows::{event::ManualResetEvent, process, thread},
};
use std::{collections::VecDeque, mem::MaybeUninit, sync::Mutex, time::Duration};

static mut LOG_MESSAGE_SENDER: MaybeUninit<Mutex<Sender<message::Log>>> = MaybeUninit::uninit();
static mut IDLE_MESSAGE_SENDER: MaybeUninit<Mutex<Sender<message::Idle>>> = MaybeUninit::uninit();

#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
    SetMousePosition { x: u16, y: u16 },
    SetMouseButtonState { button: MouseButton, state: bool },
    Idle,
}

struct EventQueueInner {
    queue: VecDeque<Event>,
    pending_event: ManualResetEvent,
}

pub struct EventQueue(Mutex<EventQueueInner>);

impl EventQueue {
    #[must_use]
    pub fn new() -> Self {
        Self(Mutex::new(EventQueueInner {
            queue: VecDeque::new(),
            pending_event: ManualResetEvent::new().unwrap(),
        }))
    }

    pub fn enqueue(&self, event: Event) {
        let mut inner = self.0.lock().unwrap();
        inner.queue.push_back(event);
        inner.pending_event.set().unwrap();
    }

    pub fn dequeue_blocking(&self) -> Event {
        let mut inner = self.0.lock().unwrap();
        if inner.queue.is_empty() {
            let pending_event = inner.pending_event.try_clone().unwrap();
            drop(inner);
            block_on(pending_event.wait()).unwrap();
            inner = self.0.lock().unwrap();
        }
        let event = inner.queue.pop_front().unwrap();
        if inner.queue.is_empty() {
            inner.pending_event.reset().unwrap();
        }
        event
    }
}

impl Default for EventQueue {
    fn default() -> Self {
        Self::new()
    }
}

static mut EVENT_QUEUE: MaybeUninit<EventQueue> = MaybeUninit::uninit();

macro_rules! log {
    ($level:expr, $($format_args:expr $(,)?),+) => {
        let log_message_sender = unsafe { crate::LOG_MESSAGE_SENDER.assume_init_ref() };
        futures::executor::block_on(log_message_sender
            .lock()
            .unwrap()
            .send(shared::ipc::message::Log {
                level: $level,
                message: format!($($format_args),+),
            }))
            .unwrap();
    };
}
pub(crate) use log;

#[expect(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "system" fn initialize(initial_message_pointer: *mut u8) {
    let mut initialized_message_sender;
    let mut message_receiver;

    unsafe {
        let initial_message_len = u32::from_ne_bytes(
            (std::slice::from_raw_parts(initial_message_pointer, size_of::<u32>()))
                .try_into()
                .unwrap(),
        )
        .try_into()
        .unwrap();
        let initial_message = message::Initial::deserialize(std::slice::from_raw_parts(
            initial_message_pointer.byte_add(size_of::<u32>()),
            initial_message_len,
        ))
        .unwrap();
        process::Process::get_current()
            .free_memory(initial_message_pointer as usize)
            .unwrap();
        initialized_message_sender = initial_message.initialized_message_sender;
        LOG_MESSAGE_SENDER = MaybeUninit::new(Mutex::new(initial_message.log_message_sender));
        IDLE_MESSAGE_SENDER = MaybeUninit::new(Mutex::new(initial_message.idle_message_sender));
        message_receiver = initial_message.message_receiver;
        state::MAIN_THREAD_ID.write(initial_message.main_thread_id);
        EVENT_QUEUE.write(EventQueue::new());
    }

    std::panic::set_hook(Box::new(|panic_info| {
        log!(
            LogLevel::Error,
            "panicked{}{}",
            match panic_info.location() {
                Some(location) => format!(" at {location}"),
                None => String::new(),
            },
            if let Some(payload) = panic_info.payload().downcast_ref::<&str>() {
                format!(": {payload}")
            } else if let Some(payload) = panic_info.payload().downcast_ref::<String>() {
                format!(": {payload}")
            } else {
                String::new()
            }
        );
    }));

    hooks::initialize();

    block_on(initialized_message_sender.send(message::Initialized)).unwrap();

    log!(
        LogLevel::Debug,
        "assuming thread with id {:#x} to be the main thread",
        unsafe { state::MAIN_THREAD_ID.assume_init_ref() }
    );
    loop {
        let event_queue = unsafe { EVENT_QUEUE.assume_init_ref() };
        match block_on(message_receiver.receive()).unwrap() {
            message::FromConductor::Resume => {
                for thread in process::Process::get_current()
                    .iter_thread_ids()
                    .unwrap()
                    .map(thread::Thread::from_id)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
                {
                    thread.decrement_suspend_count().unwrap();
                }
            }
            message::FromConductor::AdvanceTime(duration) => {
                event_queue.enqueue(Event::AdvanceTime(duration));
            }
            message::FromConductor::SetKeyState { id, state } => {
                event_queue.enqueue(Event::SetKeyState { id, state });
            }
            message::FromConductor::SetMousePosition { x, y } => {
                event_queue.enqueue(Event::SetMousePosition { x, y });
            }
            message::FromConductor::SetMouseButtonState { button, state } => {
                event_queue.enqueue(Event::SetMouseButtonState { button, state });
            }
            message::FromConductor::IdleRequest => {
                event_queue.enqueue(Event::Idle);
            }
            message => unimplemented!("handle message {message:?}"),
        }
    }
}
