use std::sync::Arc;

use simple_confidence_monitor::autopilot::advance_expired;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::room::{Command, Room, RoomName};
use simple_confidence_monitor::timer::{Mode, Run};

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn room_with_cues(hub: &Hub, name: &str) -> (Arc<Room>, Vec<u64>) {
    let room = hub.get_or_create(&RoomName::parse(name).unwrap());
    for (title, minutes) in [("Welcome", 5), ("Keynote", 30)] {
        room.apply(
            &Command::AddCue {
                title: Some(title.to_string()),
                speaker: None,
                duration_ms: Some(minutes * MIN),
                notes: None,
            },
            T0,
        );
    }
    let ids = room
        .snapshot()
        .rundown
        .cues
        .iter()
        .map(|cue| cue.id)
        .collect();
    (room, ids)
}

/// A room on its first cue, running, with auto advance on.
fn running_room(hub: &Hub, name: &str) -> (Arc<Room>, Vec<u64>) {
    let (room, ids) = room_with_cues(hub, name);
    room.apply(&Command::SetAutoAdvance { on: true }, T0);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    room.apply(&Command::Start, T0);
    (room, ids)
}

#[test]
fn an_expired_cue_advances_and_starts_the_next_one() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");

    assert_eq!(advance_expired(&hub, T0 + 5 * MIN), 1);
    let state = room.snapshot();
    assert_eq!(state.rundown.active, Some(ids[1]));
    assert_eq!(state.timer.duration_ms, 30 * MIN);
    assert_eq!(
        state.timer.run,
        Run::Running {
            since_ms: T0 + 5 * MIN
        }
    );
    assert_eq!(state.display.title, "Keynote");
}

#[test]
fn a_cue_with_time_left_stays_put() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");
    assert_eq!(advance_expired(&hub, T0 + 4 * MIN), 0);
    assert_eq!(room.snapshot().rundown.active, Some(ids[0]));
}

#[test]
fn auto_advance_off_never_advances() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");
    room.apply(&Command::SetAutoAdvance { on: false }, T0);
    assert_eq!(advance_expired(&hub, T0 + 10 * MIN), 0);
    assert_eq!(room.snapshot().rundown.active, Some(ids[0]));
}

#[test]
fn a_paused_room_never_advances() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");
    room.apply(&Command::Pause, T0 + MIN);
    assert_eq!(advance_expired(&hub, T0 + 10 * MIN), 0);
    assert_eq!(room.snapshot().rundown.active, Some(ids[0]));
}

#[test]
fn the_last_cue_stays_in_overtime() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");
    room.apply(&Command::LoadCue { id: ids[1] }, T0);
    room.apply(&Command::Start, T0);
    assert_eq!(advance_expired(&hub, T0 + 40 * MIN), 0);
    let state = room.snapshot();
    assert_eq!(state.rundown.active, Some(ids[1]));
    assert!(state.timer.readout(T0 + 40 * MIN).value_ms < 0);
}

#[test]
fn a_count_up_timer_never_advances() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");
    room.apply(
        &Command::SetMode {
            mode: Mode::CountUp,
        },
        T0,
    );
    room.apply(&Command::Start, T0);
    assert_eq!(advance_expired(&hub, T0 + 10 * MIN), 0);
    assert_eq!(room.snapshot().rundown.active, Some(ids[0]));
}

#[test]
fn every_expired_room_advances_in_one_pass() {
    let hub = Hub::new();
    running_room(&hub, "one");
    running_room(&hub, "two");
    hub.get_or_create(&RoomName::parse("idle").unwrap());
    assert_eq!(advance_expired(&hub, T0 + 5 * MIN), 2);
}

#[test]
fn an_advance_lands_in_one_step() {
    let hub = Hub::new();
    let (room, ids) = running_room(&hub, "keynote");
    let mut frames = room.subscribe();

    assert_eq!(advance_expired(&hub, T0 + 5 * MIN), 1);

    // One command, one frame: a console must never see a half-advanced room.
    let first = frames.try_recv().expect("an advance should publish once");
    let state: serde_json::Value = serde_json::from_str(&first).unwrap();
    assert_eq!(state["rundown"]["active"], ids[1]);
    assert_eq!(state["timer"]["run"]["state"], "running");
    assert!(
        frames.try_recv().is_err(),
        "a second frame means the room was visible mid-advance"
    );
}
