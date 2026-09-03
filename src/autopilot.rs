use std::time::Duration;

use crate::hub::Hub;
use crate::room::{Command, Room, RoomState};
use crate::timer::Mode;

/// How often the autopilot looks for a cue that ran out.
pub const SCAN_INTERVAL: Duration = Duration::from_millis(200);

/// What one scan found for one room.
enum Move {
    Nothing,
    Start,
    Advance { active: u64, next: u64 },
}

/// Starts an armed room at its appointed time, and starts the next cue in every
/// room whose running cue reached zero. Returns how many rooms it moved.
///
/// The readout needs no clock of its own, so only auto advance scans.
pub fn advance_expired(hub: &Hub, now_ms: u64) -> usize {
    let mut advanced = 0;
    for name in hub.room_names() {
        let Some(room) = hub.get(&name) else { continue };
        if advance_room(&room, now_ms) {
            advanced += 1;
        }
    }
    advanced
}

/// The decision comes from a read and is re-checked under the write lock, so an
/// operator who disarms or reloads inside that window is not overridden.
fn advance_room(room: &Room, now_ms: u64) -> bool {
    match room.peek(|state| next_move(state, now_ms)) {
        Move::Nothing => false,
        Move::Start => room.apply_if(|state| is_due(state, now_ms), &[Command::Start], now_ms),
        Move::Advance { active, next } => room.apply_if(
            |state| ran_out(state, now_ms) && follows(state, active, next),
            &[Command::LoadCue { id: next }, Command::Start],
            now_ms,
        ),
    }
}

fn next_move(state: &RoomState, now_ms: u64) -> Move {
    if is_due(state, now_ms) {
        return Move::Start;
    }
    if !ran_out(state, now_ms) {
        return Move::Nothing;
    }
    let (Some(active), Some(position)) = (state.rundown.active, state.rundown.active_position())
    else {
        return Move::Nothing;
    };
    match state.rundown.cues.get(position + 1) {
        Some(next) => Move::Advance {
            active,
            next: next.id,
        },
        None => Move::Nothing,
    }
}

/// The room still sits on `active`, and `next` still comes after it. An edit
/// inside the window changes which cue is next, and the scan takes none.
fn follows(state: &RoomState, active: u64, next: u64) -> bool {
    state.rundown.active == Some(active)
        && state
            .rundown
            .active_position()
            .and_then(|position| state.rundown.cues.get(position + 1))
            .map(|cue| cue.id)
            == Some(next)
}

/// An armed start whose clock time has come.
fn is_due(state: &RoomState, now_ms: u64) -> bool {
    state
        .timer
        .start_at_ms
        .is_some_and(|at_ms| !state.timer.is_running() && now_ms >= at_ms)
}

/// A running countdown cue that reached zero, in a room set to auto advance.
fn ran_out(state: &RoomState, now_ms: u64) -> bool {
    state.rundown.auto_advance
        && state.timer.mode == Mode::Countdown
        && state.timer.is_running()
        && state.timer.readout(now_ms).value_ms <= 0
}

pub async fn run(hub: &Hub, interval: Duration) {
    let mut ticker = tokio::time::interval(interval);
    loop {
        ticker.tick().await;
        advance_expired(hub, crate::clock::now_ms());
    }
}
