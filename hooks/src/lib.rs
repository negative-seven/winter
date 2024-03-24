#![allow(clippy::missing_panics_doc)]

mod hooks;
mod state;

use shared::{
    communication::{self, ConductorInitialMessage, ConductorMessage, HooksMessage, LogLevel},
    event::ManualResetEvent,
    process, thread,
};
use std::{collections::VecDeque, mem::MaybeUninit, sync::Mutex, time::Duration};

static mut MESSAGE_SENDER: MaybeUninit<Mutex<communication::Sender<HooksMessage>>> =
    MaybeUninit::uninit();

#[derive(Debug)]
#[non_exhaustive]
pub enum Event {
    AdvanceTime(Duration),
    SetKeyState { id: u8, state: bool },
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
            pending_event.wait().unwrap();
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
    ($level:expr, $($format_args:expr $(,)?),+) => {{
        let message_sender = unsafe { crate::MESSAGE_SENDER.assume_init_ref() };
        message_sender
            .lock()
            .unwrap()
            .send(&shared::communication::HooksMessage::Log {
                level: $level,
                message: format!($($format_args),+),
            })
            .unwrap();
    }};
}
pub(crate) use log;

#[allow(clippy::missing_safety_doc)]
#[no_mangle]
pub unsafe extern "stdcall" fn initialize(initial_message_pointer: *mut ConductorInitialMessage) {
    let initial_message = std::ptr::read_unaligned(initial_message_pointer);
    process::Process::get_current()
        .free_memory(initial_message_pointer as usize)
        .unwrap();
    let mut message_receiver;
    MESSAGE_SENDER.write(Mutex::new(
        communication::Sender::<HooksMessage>::from_bytes(
            initial_message.serialized_message_sender,
        ),
    ));
    message_receiver = communication::Receiver::<ConductorMessage>::from_bytes(
        initial_message.serialized_message_receiver,
    );
    state::MAIN_THREAD_ID.write(initial_message.main_thread_id);
    EVENT_QUEUE.write(EventQueue::new());

    hooks::initialize();

    let message_sender = unsafe { MESSAGE_SENDER.assume_init_ref() };
    message_sender
        .lock()
        .unwrap()
        .send(&HooksMessage::HooksInitialized)
        .unwrap();
    log!(
        LogLevel::Debug,
        "assuming thread with id 0x{:x} to be the main thread",
        state::MAIN_THREAD_ID.assume_init_ref()
    );
    loop {
        let event_queue = unsafe { EVENT_QUEUE.assume_init_ref() };
        match message_receiver.receive_blocking().unwrap() {
            #[allow(clippy::cast_possible_truncation)]
            ConductorMessage::Resume => {
                for thread in process::Process::get_current()
                    .iter_thread_ids()
                    .unwrap()
                    .map(thread::Thread::from_id)
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap()
                {
                    thread.resume().unwrap();
                }
            }
            ConductorMessage::AdvanceTime(duration) => {
                event_queue.enqueue(Event::AdvanceTime(duration));
            }
            ConductorMessage::SetKeyState { id, state } => {
                event_queue.enqueue(Event::SetKeyState { id, state });
            }
            ConductorMessage::IdleRequest => {
                event_queue.enqueue(Event::Idle);
            }
            message => unimplemented!("handle message {message:?}"),
        }
    }
}
