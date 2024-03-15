use crate::{Event, EVENTS, MESSAGE_SENDER};
use shared::communication::HooksMessage;
use std::sync::Mutex;

pub struct State {
    pub ticks: u64,
    pub pending_ticks: u64,
    pub busy_wait_count: u64,
}

impl State {
    pub const TICKS_PER_SECOND: u64 = 3000;
}

pub static STATE: Mutex<State> = Mutex::new(State {
    ticks: 0,
    pending_ticks: 0,
    busy_wait_count: 0,
});

pub fn sleep(ticks: u64) {
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
            let events = EVENTS.read();
            let mut events_guard = events.lock().unwrap();
            let event = events_guard.queue.pop_front();
            match event {
                #[allow(clippy::cast_possible_truncation)]
                Some(Event::AdvanceTime(duration)) => {
                    STATE.lock().unwrap().pending_ticks += (duration.as_nanos()
                        * u128::from(State::TICKS_PER_SECOND)
                        / std::time::Duration::from_secs(1).as_nanos())
                        as u64;
                    break;
                }
                #[allow(unreachable_patterns)] // Event is #[non_exhaustive]
                Some(event) => unimplemented!("event {event:?}"),
                None => {
                    unsafe {
                        MESSAGE_SENDER
                            .assume_init_ref()
                            .lock()
                            .unwrap()
                            .send(&HooksMessage::Idle)
                            .unwrap();
                    }

                    let mut event_pending = events_guard.pending.try_clone().unwrap();
                    event_pending.reset().unwrap();
                    drop(events_guard);
                    event_pending.wait().unwrap();
                }
            }
        }
    }
}
