use crate::{hooks, log, Event, EVENT_QUEUE, IDLE_MESSAGE_SENDER};
use futures::executor::block_on;
use shared::communication::{IdleMessage, LogLevel, MouseButton};
use std::{
    collections::{BTreeMap, VecDeque},
    mem::MaybeUninit,
    num::NonZeroU64,
    sync::{Arc, Mutex},
};
use winapi::{
    shared::{
        ntdef::NULL,
        windef::{HWND, POINT},
    },
    um::{
        processthreadsapi::GetCurrentThreadId,
        synchapi::Sleep,
        winuser::{
            self, EnumThreadWindows, IsWindowVisible, MSG, VK_CONTROL, VK_LCONTROL, VK_LMENU,
            VK_LSHIFT, VK_RCONTROL, VK_RMENU, VK_RSHIFT, VK_SHIFT, WM_KEYDOWN, WM_KEYUP,
            WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE,
            WM_RBUTTONDOWN, WM_RBUTTONUP, WM_XBUTTONDOWN, WM_XBUTTONUP,
        },
    },
};

#[derive(Clone)]
pub(crate) struct MSGSend(pub(crate) MSG);

unsafe impl Send for MSGSend {}

#[expect(clippy::struct_excessive_bools)]
pub(crate) struct MouseState {
    pub(crate) x: u16,
    pub(crate) y: u16,
    left_button: bool,
    right_button: bool,
    middle_button: bool,
    x1_button: bool,
    x2_button: bool,
}

pub(crate) struct WaitableTimer {
    pub(crate) reset_automatically: bool,
    pub(crate) signaled: bool,
    pub(crate) remaining_ticks: u64,
    pub(crate) period_in_ticks: Option<NonZeroU64>,
}

impl WaitableTimer {
    pub(crate) fn running(&self) -> bool {
        self.remaining_ticks != 0
    }
}

pub(crate) struct State {
    ticks: u64,
    pending_ticks: u64,
    busy_wait_count: u64,
    key_states: [bool; 256],
    pub(crate) mouse: MouseState,
    pub(crate) custom_message_queue: VecDeque<MSGSend>,
    pub(crate) waitable_timer_handles: BTreeMap<u32, Arc<Mutex<WaitableTimer>>>,
}

impl State {
    pub(crate) const TICKS_PER_SECOND: u64 = 3000;
    const BUSY_WAIT_THRESHOLD: u64 = 100;

    pub(crate) fn ticks(&self) -> u64 {
        self.ticks
    }

    pub(crate) fn get_key_state(&self, key_code: u8) -> bool {
        #[expect(clippy::cast_possible_truncation)]
        const VK_SHIFT: u8 = winuser::VK_SHIFT as u8;
        #[expect(clippy::cast_possible_truncation)]
        const VK_CONTROL: u8 = winuser::VK_CONTROL as u8;
        #[expect(clippy::cast_possible_truncation)]
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

    pub(crate) fn set_mouse_button_state(&mut self, button: MouseButton, state: bool) {
        *match button {
            MouseButton::Left => &mut self.mouse.left_button,
            MouseButton::Right => &mut self.mouse.right_button,
            MouseButton::Middle => &mut self.mouse.middle_button,
            MouseButton::X1 => &mut self.mouse.x1_button,
            MouseButton::X2 => &mut self.mouse.x2_button,
        } = state;
    }
}

pub(crate) static STATE: Mutex<State> = Mutex::new(State {
    ticks: 0,
    pending_ticks: 0,
    busy_wait_count: 0,
    key_states: [false; 256],
    mouse: MouseState {
        x: 0,
        y: 0,
        left_button: false,
        right_button: false,
        middle_button: false,
        x1_button: false,
        x2_button: false,
    },
    custom_message_queue: VecDeque::new(),
    waitable_timer_handles: BTreeMap::new(),
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
            sleep_indefinitely();
            state = STATE.lock().unwrap();
        }
    }
    state.ticks
}

pub(crate) fn sleep(ticks: u64) {
    if !in_main_thread() {
        let sleep_trampoline = hooks::get_trampoline!(Sleep, unsafe extern "system" fn(u32));
        unsafe {
            #[expect(clippy::cast_possible_truncation)]
            sleep_trampoline((ticks * 1000 / State::TICKS_PER_SECOND) as u32);
        }
        return;
    }

    log!(LogLevel::Debug, "sleeping for {ticks} ticks");

    let mut remaining_ticks = ticks;
    while remaining_ticks > 0 {
        let ticks_advanced_by;
        {
            let mut state = STATE.lock().unwrap();
            ticks_advanced_by = u64::min(state.pending_ticks, remaining_ticks);
            state.ticks += ticks_advanced_by;
            state.pending_ticks -= ticks_advanced_by;
        }
        remaining_ticks -= ticks_advanced_by;
        advance_timers(ticks_advanced_by);
        if remaining_ticks == 0 {
            STATE.lock().unwrap().busy_wait_count = 0;
            break;
        }
        poll_events_for_sleep();
    }
}

pub(crate) fn sleep_indefinitely() {
    if !in_main_thread() {
        return;
    }

    log!(LogLevel::Debug, "sleeping indefinitely");

    loop {
        {
            let mut state = STATE.lock().unwrap();
            let pending_ticks = state.pending_ticks;
            if pending_ticks > 0 {
                state.ticks += pending_ticks;
                state.pending_ticks = 0;
                state.busy_wait_count = 0;
                drop(state);
                advance_timers(pending_ticks);
                break;
            }
        }
        poll_events_for_sleep();
    }
}

fn advance_timers(ticks: u64) {
    for timer in STATE.lock().unwrap().waitable_timer_handles.values() {
        let mut timer = timer.lock().unwrap();
        if timer.remaining_ticks > 0 {
            let mut remaining_ticks = ticks;
            let ticks_advanced_by = timer.remaining_ticks.min(remaining_ticks);
            timer.remaining_ticks -= ticks_advanced_by;
            remaining_ticks -= ticks_advanced_by;
            if timer.remaining_ticks == 0 {
                timer.signaled = true;
                if let Some(period_in_ticks) = timer.period_in_ticks {
                    remaining_ticks %= u64::from(period_in_ticks);
                    timer.remaining_ticks = u64::from(period_in_ticks) - remaining_ticks;
                }
            }
        }
    }
}

fn poll_events_for_sleep() {
    loop {
        let event_queue = unsafe { EVENT_QUEUE.assume_init_ref() };
        let event = event_queue.dequeue_blocking();
        match event {
            #[expect(clippy::cast_possible_truncation)]
            #[expect(clippy::cast_precision_loss)]
            #[expect(clippy::cast_sign_loss)]
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

                post_message(
                    if key_state { WM_KEYDOWN } else { WM_KEYUP },
                    usize::from(key_id),
                    (isize::from(!key_state) << 31) | (isize::from(key_previous_state) << 30) | 1,
                );
            }
            Event::SetMousePosition { x, y } => {
                {
                    let mut state = STATE.lock().unwrap();
                    state.mouse.x = x;
                    state.mouse.y = y;
                }
                post_mouse_message(WM_MOUSEMOVE, 0);
            }
            Event::SetMouseButtonState {
                button,
                state: button_state,
            } => {
                STATE
                    .lock()
                    .unwrap()
                    .set_mouse_button_state(button, button_state);
                post_mouse_message(
                    match (button, button_state) {
                        (MouseButton::Left, true) => WM_LBUTTONDOWN,
                        (MouseButton::Left, false) => WM_LBUTTONUP,
                        (MouseButton::Right, true) => WM_RBUTTONDOWN,
                        (MouseButton::Right, false) => WM_RBUTTONUP,
                        (MouseButton::Middle, true) => WM_MBUTTONDOWN,
                        (MouseButton::Middle, false) => WM_MBUTTONUP,
                        (MouseButton::X1 | MouseButton::X2, true) => WM_XBUTTONDOWN,
                        (MouseButton::X1 | MouseButton::X2, false) => WM_XBUTTONUP,
                    },
                    match button {
                        MouseButton::X1 => 1,
                        MouseButton::X2 => 2,
                        _ => 0,
                    },
                );
            }
            Event::Idle => unsafe {
                block_on(
                    IDLE_MESSAGE_SENDER
                        .assume_init_ref()
                        .lock()
                        .unwrap()
                        .send(&IdleMessage),
                )
                .unwrap();
            },
            #[expect(unreachable_patterns)] // Event is #[non_exhaustive]
            event => unimplemented!("event {event:?}"),
        }
    }
}

fn post_message(message_id: u32, w_parameter: usize, l_parameter: isize) {
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

    #[expect(clippy::cast_possible_truncation)]
    let time_in_ticks = STATE.lock().unwrap().ticks as u32;
    unsafe {
        EnumThreadWindows(
            MAIN_THREAD_ID.assume_init(),
            Some(callback),
            std::ptr::from_mut::<MSGSend>(&mut MSGSend(MSG {
                hwnd: NULL.cast(),
                message: message_id,
                wParam: w_parameter,
                lParam: l_parameter,
                time: time_in_ticks,
                pt: POINT { x: 0, y: 0 },
            })) as isize,
        );
    }
}

fn post_mouse_message(message_id: u32, w_parameter_high_word: u16) {
    let w_parameter;
    let l_parameter;
    #[expect(clippy::cast_possible_truncation)]
    #[expect(clippy::cast_possible_wrap)]
    {
        let state = STATE.lock().unwrap();
        w_parameter = (usize::from(w_parameter_high_word) << 16)
            | (usize::from(state.mouse.x2_button) << 6)
            | (usize::from(state.mouse.x1_button) << 5)
            | (usize::from(state.mouse.middle_button) << 4)
            | (usize::from(state.get_key_state(VK_CONTROL as u8)) << 3)
            | (usize::from(state.get_key_state(VK_SHIFT as u8)) << 2)
            | (usize::from(state.mouse.right_button) << 1)
            | usize::from(state.mouse.left_button);
        l_parameter = ((state.mouse.y as isize) << 16) | (state.mouse.x as isize);
    }
    post_message(message_id, w_parameter, l_parameter);
}
