use shared::{communication::HooksMessage, event::ManualResetEvent};
use static_init::dynamic;
use std::sync::Mutex;

use crate::TRANSCEIVER;

pub struct State {
    pub ticks: u64,
    pub pending_ticks: u64,
    pub busy_wait_count: u64,
}

impl State {
    pub const TICKS_PER_SECOND: u64 = 3000;

    pub fn sleep(this: &Mutex<Self>, ticks: u64) {
        let mut remaining_ticks = ticks;
        loop {
            let mut this_guard = this.lock().unwrap();

            let ticks_advanced_by = u64::min(this_guard.pending_ticks, remaining_ticks);
            this_guard.ticks += ticks_advanced_by;
            remaining_ticks -= ticks_advanced_by;
            this_guard.pending_ticks -= ticks_advanced_by;

            if remaining_ticks == 0 {
                break;
            }

            drop(this_guard);

            unsafe {
                TRANSCEIVER
                    .assume_init_ref()
                    .send(&HooksMessage::Idle)
                    .unwrap();
            }

            let mut ticks_pending_event = TICKS_PENDING_EVENT.read().try_clone().unwrap();
            ticks_pending_event.wait().unwrap();
            ticks_pending_event.reset().unwrap();
        }
    }
}

pub static STATE: Mutex<State> = Mutex::new(State {
    ticks: 0,
    pending_ticks: 0,
    busy_wait_count: 0,
});
#[dynamic]
pub static mut TICKS_PENDING_EVENT: ManualResetEvent = ManualResetEvent::new().unwrap();
