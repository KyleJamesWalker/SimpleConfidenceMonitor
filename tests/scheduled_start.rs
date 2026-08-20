use simple_confidence_monitor::autopilot::advance_expired;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::room::{Command, Room, RoomName};
use simple_confidence_monitor::timer::Run;

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn arm(at_ms: Option<u64>) -> Command {
    Command::ScheduleStart { at_ms }
}

#[test]
fn a_new_room_has_nothing_armed() {
    assert_eq!(Room::default().snapshot().timer.start_at_ms, None);
}

#[test]
fn arming_a_start_records_the_time() {
    let room = Room::default();
    let state = room.apply(&arm(Some(T0 + 10 * MIN)), T0);
    assert_eq!(state.timer.start_at_ms, Some(T0 + 10 * MIN));
}

#[test]
fn arming_a_start_leaves_the_timer_stopped_at_the_top() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    let state = room.apply(&arm(Some(T0 + 10 * MIN)), T0 + 2 * MIN);
    assert_eq!(state.timer.run, Run::Stopped);
    assert_eq!(state.timer.elapsed_ms, 0);
}

#[test]
fn arming_the_same_time_twice_does_not_bump_rev() {
    let room = Room::default();
    let first = room.apply(&arm(Some(T0 + MIN)), T0);
    assert_eq!(room.apply(&arm(Some(T0 + MIN)), T0).rev, first.rev);
}

#[test]
fn arming_nothing_cancels_a_pending_start() {
    let room = Room::default();
    room.apply(&arm(Some(T0 + MIN)), T0);
    assert_eq!(room.apply(&arm(None), T0).timer.start_at_ms, None);
}

#[test]
fn starting_by_hand_cancels_a_pending_start() {
    let room = Room::default();
    room.apply(&arm(Some(T0 + 10 * MIN)), T0);
    let state = room.apply(&Command::Start, T0 + MIN);
    assert_eq!(state.timer.start_at_ms, None);
    assert_eq!(state.timer.run, Run::Running { since_ms: T0 + MIN });
}

#[test]
fn reset_cancels_a_pending_start() {
    let room = Room::default();
    room.apply(&arm(Some(T0 + 10 * MIN)), T0);
    assert_eq!(room.apply(&Command::Reset, T0).timer.start_at_ms, None);
}

#[test]
fn loading_a_cue_cancels_a_pending_start() {
    let room = Room::default();
    room.apply(
        &Command::AddCue {
            title: Some("Welcome".into()),
            speaker: None,
            duration_ms: Some(5 * MIN),
            notes: None,
        },
        T0,
    );
    let id = room.snapshot().rundown.cues[0].id;
    room.apply(&arm(Some(T0 + 10 * MIN)), T0);
    assert_eq!(
        room.apply(&Command::LoadCue { id }, T0).timer.start_at_ms,
        None
    );
}

#[test]
fn the_autopilot_starts_an_armed_room_at_its_time() {
    let hub = Hub::new();
    let room = hub.get_or_create(&RoomName::parse("keynote").unwrap());
    room.apply(&arm(Some(T0 + 5 * MIN)), T0);

    assert_eq!(advance_expired(&hub, T0 + 5 * MIN - 1), 0);
    assert_eq!(room.snapshot().timer.run, Run::Stopped);

    assert_eq!(advance_expired(&hub, T0 + 5 * MIN), 1);
    let state = room.snapshot();
    assert_eq!(
        state.timer.run,
        Run::Running {
            since_ms: T0 + 5 * MIN
        }
    );
    assert_eq!(state.timer.start_at_ms, None);
}

#[test]
fn a_room_that_is_already_running_ignores_an_armed_start() {
    let hub = Hub::new();
    let room = hub.get_or_create(&RoomName::parse("keynote").unwrap());
    room.apply(&arm(Some(T0 + MIN)), T0);
    room.apply(&Command::Start, T0);
    assert_eq!(advance_expired(&hub, T0 + 2 * MIN), 0);
    assert_eq!(room.snapshot().timer.run, Run::Running { since_ms: T0 });
}

#[test]
fn a_time_already_past_fires_on_the_next_pass() {
    let hub = Hub::new();
    let room = hub.get_or_create(&RoomName::parse("keynote").unwrap());
    room.apply(&arm(Some(T0 - 10 * MIN)), T0);
    assert_eq!(advance_expired(&hub, T0), 1);
    assert!(room.snapshot().timer.is_running());
}

#[test]
fn parses_the_schedule_commands() {
    let armed: Command =
        serde_json::from_str(r#"{"cmd":"schedule_start","at_ms":1700000000000}"#).unwrap();
    assert_eq!(armed, arm(Some(T0)));
    let cleared: Command = serde_json::from_str(r#"{"cmd":"schedule_start"}"#).unwrap();
    assert_eq!(cleared, arm(None));
}
