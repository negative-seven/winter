use crate::{log, Event, EVENT_QUEUE, MESSAGE_SENDER};
use shared::communication::{HooksMessage, LogLevel};
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
