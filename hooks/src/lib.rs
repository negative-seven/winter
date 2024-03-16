#![allow(clippy::missing_panics_doc)]

mod hooks;
mod state;

use hooks::{
    get_async_key_state, get_key_state, get_keyboard_state, get_tick_count, peek_message,
    query_performance_counter, query_performance_frequency, sleep, time_get_time,
};
use minhook::MinHook;
use shared::{
    communication::{self, HooksMessage, RuntimeMessage},
    event::ManualResetEvent,
    process,
};
use static_init::dynamic;
use std::{
    collections::{HashMap, VecDeque},
    error::Error,
    ffi::c_void,
    mem::MaybeUninit,
    sync::Mutex,
    time::Duration,
};

#[allow(clippy::ignored_unit_patterns)] // lint triggered inside macro
#[dynamic]
static mut TRAMPOLINES: HashMap<String, usize> = HashMap::new();

unsafe fn get_trampoline(function_name: impl AsRef<str>) -> *const c_void {
    *TRAMPOLINES.read().get(function_name.as_ref()).unwrap() as *const c_void
}

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
    ($level:expr, $($format_args:expr),+) => {
        let message_sender = unsafe { crate::MESSAGE_SENDER.assume_init_ref() };
        message_sender
            .lock()
            .unwrap()
            .send(&shared::communication::HooksMessage::Log {
                level: $level,
                message: format!($($format_args),+),
            })
            .unwrap();
    };
}
pub(crate) use log;

fn hook_function(
    module_name: &str,
    function_name: &str,
    hook: *const c_void,
) -> Result<(), Box<dyn Error>> {
    let process = process::Process::get_current();
    let function_address = process.get_export_address(module_name, function_name)?;
    unsafe {
        #[allow(clippy::ptr_cast_constness)] // the pointer being mutable seems to be pointless
        let original_function =
            MinHook::create_hook(function_address as *mut c_void, hook as *mut c_void).unwrap();
        MinHook::enable_hook(function_address as *mut c_void).unwrap();
        TRAMPOLINES
            .write()
            .insert(function_name.to_string(), original_function as usize);
    }
    Ok(())
}

#[no_mangle]
pub extern "stdcall" fn initialize(serialized_sender_and_receiver_pointer: usize) {
    let mut message_receiver;
    unsafe {
        MESSAGE_SENDER.write(Mutex::new(
            communication::Sender::<HooksMessage>::from_bytes(
                process::Process::get_current()
                    .read_to_vec(serialized_sender_and_receiver_pointer, 12)
                    .unwrap()
                    .try_into()
                    .unwrap(),
            ),
        ));
        message_receiver = communication::Receiver::<RuntimeMessage>::from_bytes(
            process::Process::get_current()
                .read_to_vec(serialized_sender_and_receiver_pointer + 12, 12)
                .unwrap()
                .try_into()
                .unwrap(),
        );

        EVENT_QUEUE.write(EventQueue::new());
    }

    for (module_name, function_name, hook) in [
        (
            "user32.dll",
            "GetKeyboardState",
            get_keyboard_state as *const c_void,
        ),
        ("user32.dll", "GetKeyState", get_key_state as *const c_void),
        (
            "user32.dll",
            "GetAsyncKeyState",
            get_async_key_state as *const c_void,
        ),
        ("kernel32.dll", "Sleep", sleep as *const c_void),
        ("user32.dll", "PeekMessageA", peek_message as *const c_void),
        (
            "kernel32.dll",
            "GetTickCount",
            get_tick_count as *const c_void,
        ),
        ("winmm.dll", "timeGetTime", time_get_time as *const c_void),
        (
            "kernel32.dll",
            "QueryPerformanceFrequency",
            query_performance_frequency as *const c_void,
        ),
        (
            "kernel32.dll",
            "QueryPerformanceCounter",
            query_performance_counter as *const c_void,
        ),
    ] {
        let _ = hook_function(module_name, function_name, hook);
    }

    let message_sender = unsafe { MESSAGE_SENDER.assume_init_ref() };
    message_sender
        .lock()
        .unwrap()
        .send(&HooksMessage::HooksInitialized)
        .unwrap();
    loop {
        let event_queue = unsafe { EVENT_QUEUE.assume_init_ref() };
        match message_receiver.receive_blocking().unwrap() {
            #[allow(clippy::cast_possible_truncation)]
            RuntimeMessage::AdvanceTime(duration) => {
                event_queue.enqueue(Event::AdvanceTime(duration));
            }
            RuntimeMessage::SetKeyState { id, state } => {
                event_queue.enqueue(Event::SetKeyState { id, state });
            }
            RuntimeMessage::IdleRequest => {
                event_queue.enqueue(Event::Idle);
            }
            message => unimplemented!("handle message {message:?}"),
        }
    }
}
