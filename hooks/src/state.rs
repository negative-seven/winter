use crate::{log, Event, EVENT_QUEUE, MESSAGE_SENDER};
use shared::{
    communication::{HooksMessage, LogLevel},
    process,
};
use std::{collections::VecDeque, sync::Mutex};
use winapi::{
    shared::{
        ntdef::NULL,
        windef::{HWND, POINT},
    },
    um::winuser::{EnumThreadWindows, MSG, WM_KEYDOWN, WM_KEYUP},
};

#[derive(Clone)]
pub struct MSGSend(pub MSG);

unsafe impl Send for MSGSend {}

pub struct State {
    ticks: u64,
    pending_ticks: u64,
    busy_wait_count: u64,
    pub key_states: [bool; 256],
    pub custom_message_queue: VecDeque<MSGSend>,
}

impl State {
    pub const TICKS_PER_SECOND: u64 = 3000;
    const BUSY_WAIT_THRESHOLD: u64 = 100;
}

pub static STATE: Mutex<State> = Mutex::new(State {
    ticks: 0,
    pending_ticks: 0,
    busy_wait_count: 0,
    key_states: [false; 256],
    custom_message_queue: VecDeque::new(),
});

pub fn get_ticks_with_busy_wait() -> u64 {
    let mut state = STATE.lock().unwrap();
    state.busy_wait_count += 1;
    if state.busy_wait_count >= State::BUSY_WAIT_THRESHOLD {
        drop(state);
        sleep(State::TICKS_PER_SECOND / 60);
        state = STATE.lock().unwrap();
        state.busy_wait_count = 0;
    }
    state.ticks
}

pub fn sleep(ticks: u64) {
    log!(LogLevel::Debug, "sleeping for {ticks} ticks");

    let mut remaining_ticks = ticks;
    while remaining_ticks > 0 {
        {
            let mut state = STATE.lock().unwrap();
            let ticks_advanced_by = u64::min(state.pending_ticks, remaining_ticks);
            state.ticks += ticks_advanced_by;
            state.pending_ticks -= ticks_advanced_by;
            remaining_ticks -= ticks_advanced_by;
        }

        if remaining_ticks == 0 {
            break;
        }

        loop {
            let event_queue = unsafe { EVENT_QUEUE.assume_init_ref() };
            let event = event_queue.dequeue_blocking();
            match event {
                #[allow(clippy::cast_possible_truncation)]
                Event::AdvanceTime(duration) => {
                    STATE.lock().unwrap().pending_ticks += (duration.as_nanos()
                        * u128::from(State::TICKS_PER_SECOND)
                        / std::time::Duration::from_secs(1).as_nanos())
                        as u64;

                    break;
                }
                Event::SetKeyState {
                    id: key_id,
                    state: key_state,
                } => {
                    let key_previous_state = std::mem::replace(
                        STATE
                            .lock()
                            .unwrap()
                            .key_states
                            .get_mut(usize::from(key_id))
                            .unwrap(),
                        key_state,
                    );
                    for thread_id in process::Process::get_current().iter_thread_ids().unwrap() {
                        unsafe extern "system" fn callback(window: HWND, message: isize) -> i32 {
                            let message = &mut *(message as *mut MSGSend);
                            message.0.hwnd = window;
                            STATE
                                .lock()
                                .unwrap()
                                .custom_message_queue
                                .push_back(message.clone());
                            1
                        }

                        #[allow(clippy::cast_possible_truncation)]
                        let time_in_ticks = STATE.lock().unwrap().ticks as u32;
                        unsafe {
                            EnumThreadWindows(
                                thread_id,
                                Some(callback),
                                &mut MSGSend(MSG {
                                    hwnd: NULL.cast(),
                                    message: if key_state { WM_KEYDOWN } else { WM_KEYUP },
                                    wParam: usize::from(key_id),
                                    lParam: (isize::from(!key_state) << 31)
                                        | (isize::from(key_previous_state) << 30)
                                        | 1,
                                    #[allow(clippy::cast_possible_truncation)]
                                    time: (u64::from(time_in_ticks) * 1000
                                        / State::TICKS_PER_SECOND)
                                        as u32,
                                    pt: POINT { x: 0, y: 0 },
                                }) as *mut MSGSend as isize,
                            );
                        }
                    }
                }
                Event::Idle => unsafe {
                    MESSAGE_SENDER
                        .assume_init_ref()
                        .lock()
                        .unwrap()
                        .send(&HooksMessage::Idle)
                        .unwrap();
                },
                #[allow(unreachable_patterns)] // Event is #[non_exhaustive]
                event => unimplemented!("event {event:?}"),
            }
        }
    }
}
