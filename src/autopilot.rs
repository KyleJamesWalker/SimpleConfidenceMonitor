use std::time::Duration;

use crate::hub::Hub;
use crate::room::Command;
use crate::timer::Mode;

/// How often the autopilot looks for a cue that ran out.
pub const SCAN_INTERVAL: Duration = Duration::from_millis(200);

/// Starts an armed room at its appointed time, and starts the next cue in every
/// room whose running cue reached zero. Returns how many rooms it moved.
///
/// The readout needs no clock of its own, so only auto advance scans.
pub fn advance_expired(hub: &Hub, now_ms: u64) -> usize {
    let mut advanced = 0;
    for name in hub.room_names() {
        let Some(room) = hub.get(&name) else { continue };
        let state = room.snapshot();
        if state
            .timer
            .start_at_ms
            .is_some_and(|at_ms| !state.timer.is_running() && now_ms >= at_ms)
        {
            room.apply(&Command::Start, now_ms);
            advanced += 1;
            continue;
        }
        if !state.rundown.auto_advance || state.timer.mode != Mode::Countdown {
            continue;
        }
        if !state.timer.is_running() || state.timer.readout(now_ms).value_ms > 0 {
            continue;
        }
        let Some(position) = state.rundown.active_position() else {
            continue;
        };
        let Some(next) = state.rundown.cues.get(position + 1) else {
            continue;
        };
        room.apply(&Command::LoadCue { id: next.id }, now_ms);
        room.apply(&Command::Start, now_ms);
        advanced += 1;
    }
    advanced
}

pub async fn run(hub: &Hub, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        advance_expired(hub, crate::clock::now_ms());
    }
}
