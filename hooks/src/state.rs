use crate::{log, Event, EVENT_QUEUE, MESSAGE_SENDER};
use shared::communication::{HooksMessage, LogLevel};
use std::{collections::VecDeque, mem::MaybeUninit, sync::Mutex};
use winapi::{
    shared::{
        ntdef::NULL,
        windef::{HWND, POINT},
    },
    um::{
        processthreadsapi::GetCurrentThreadId,
        winuser::{
            self, EnumThreadWindows, IsWindowVisible, MSG, VK_LCONTROL, VK_LMENU, VK_LSHIFT,
            VK_RCONTROL, VK_RMENU, VK_RSHIFT, WM_KEYDOWN, WM_KEYUP,
        },
    },
};

#[derive(Clone)]
pub(crate) struct MSGSend(pub(crate) MSG);

unsafe impl Send for MSGSend {}

pub(crate) struct State {
    ticks: u64,
    pending_ticks: u64,
    busy_wait_count: u64,
    key_states: [bool; 256],
    pub(crate) custom_message_queue: VecDeque<MSGSend>,
}

impl State {
    pub(crate) const TICKS_PER_SECOND: u64 = 3000;
    const BUSY_WAIT_THRESHOLD: u64 = 100;

    pub(crate) fn get_key_state(&self, key_code: u8) -> bool {
        #[allow(clippy::cast_possible_truncation)]
        const VK_SHIFT: u8 = winuser::VK_SHIFT as u8;
        #[allow(clippy::cast_possible_truncation)]
        const VK_CONTROL: u8 = winuser::VK_CONTROL as u8;
        #[allow(clippy::cast_possible_truncation)]
        const VK_MENU: u8 = winuser::VK_MENU as u8;

        match key_code {
            VK_SHIFT => self.key_states[VK_LSHIFT as usize] || self.key_states[VK_RSHIFT as usize],
            VK_CONTROL => {
                self.key_states[VK_LCONTROL as usize] || self.key_states[VK_RCONTROL as usize]
            }
            VK_MENU => self.key_states[VK_LMENU as usize] || self.key_states[VK_RMENU as usize],
            key_code => self.key_states[usize::from(key_code)],
        }
    }

    pub(crate) fn set_key_state(&mut self, key_code: u8, state: bool) {
        self.key_states[usize::from(key_code)] = state;
    }
}

pub(crate) static STATE: Mutex<State> = Mutex::new(State {
    ticks: 0,
    pending_ticks: 0,
    busy_wait_count: 0,
    key_states: [false; 256],
    custom_message_queue: VecDeque::new(),
});

pub(crate) static mut MAIN_THREAD_ID: MaybeUninit<u32> = MaybeUninit::uninit();
fn in_main_thread() -> bool {
    unsafe { GetCurrentThreadId() == MAIN_THREAD_ID.assume_init() }
}

pub(crate) fn get_ticks_with_busy_wait() -> u64 {
    let mut state = STATE.lock().unwrap();
    if in_main_thread() {
        state.busy_wait_count += 1;
        if state.busy_wait_count >= State::BUSY_WAIT_THRESHOLD {
            drop(state);
            sleep(State::TICKS_PER_SECOND / 60);
            state = STATE.lock().unwrap();
        }
    }
    state.ticks
}

pub(crate) fn sleep(ticks: u64) {
    if !in_main_thread() {
        return;
    }

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
            STATE.lock().unwrap().busy_wait_count = 0;
            break;
        }

        loop {
            let event_queue = unsafe { EVENT_QUEUE.assume_init_ref() };
            let event = event_queue.dequeue_blocking();
            match event {
                #[allow(clippy::cast_possible_truncation)]
                #[allow(clippy::cast_precision_loss)]
                #[allow(clippy::cast_sign_loss)]
                Event::AdvanceTime(duration) => {
                    STATE.lock().unwrap().pending_ticks +=
                        (duration.as_secs_f64() * State::TICKS_PER_SECOND as f64).round() as u64;
                    break;
                }
                Event::SetKeyState {
                    id: key_id,
                    state: key_state,
                } => {
                    let key_previous_state;
                    {
                        let mut state = STATE.lock().unwrap();
                        key_previous_state = state.get_key_state(key_id);
                        state.set_key_state(key_id, key_state);
                    }

                    {
                        unsafe extern "system" fn callback(window: HWND, message: isize) -> i32 {
                            if window != NULL.cast() && unsafe { IsWindowVisible(window) } == 0 {
                                return 1;
                            }

                            let message = unsafe { &mut *(message as *mut MSGSend) };
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
                                MAIN_THREAD_ID.assume_init(),
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
