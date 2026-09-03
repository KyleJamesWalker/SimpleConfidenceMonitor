use simple_confidence_monitor::room::{Command, Room};
use simple_confidence_monitor::timer::{Mode, OnExpire, Run};

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn parse(json: &str) -> Command {
    serde_json::from_str(json).expect("command should parse")
}

#[test]
fn a_new_room_starts_at_rev_zero() {
    let room = Room::default();
    assert_eq!(room.snapshot().rev, 0);
}

#[test]
fn a_command_that_changes_state_bumps_rev() {
    let room = Room::default();
    assert_eq!(room.apply(&Command::Start, T0).rev, 1);
    assert_eq!(room.apply(&Command::Pause, T0 + MIN).rev, 2);
}

#[test]
fn a_command_that_changes_nothing_leaves_rev_alone() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    assert_eq!(room.apply(&Command::Start, T0 + MIN).rev, 1);
}

#[test]
fn a_command_that_changes_nothing_wakes_no_client() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    let mut frames = room.subscribe();
    room.apply(&Command::Start, T0 + MIN);
    assert!(
        frames.try_recv().is_err(),
        "a command that changes nothing must wake no screen"
    );
}

#[test]
fn a_command_that_changes_state_wakes_every_client() {
    let room = Room::default();
    let mut frames = room.subscribe();
    room.apply(&Command::Start, T0);
    assert!(frames.try_recv().is_ok());
}

#[test]
fn re_asserting_blackout_wakes_no_client() {
    let room = Room::default();
    room.apply(&Command::Blackout { on: true }, T0);
    let mut frames = room.subscribe();
    room.apply(&Command::Blackout { on: true }, T0 + MIN);
    assert!(frames.try_recv().is_err());
}

#[test]
fn apply_if_runs_while_the_state_still_matches() {
    let room = Room::default();
    let applied = room.apply_if(|state| !state.timer.is_running(), &[Command::Start], T0);
    assert!(applied);
    assert!(room.snapshot().timer.is_running());
}

#[test]
fn apply_if_leaves_a_room_that_moved_on_alone() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    let mut frames = room.subscribe();
    let applied = room.apply_if(
        |state| !state.timer.is_running(),
        &[Command::Pause],
        T0 + MIN,
    );
    assert!(
        !applied,
        "the predicate no longer holds, so nothing applies"
    );
    assert_eq!(room.snapshot().timer.run, Run::Running { since_ms: T0 });
    assert!(frames.try_recv().is_err());
}

#[test]
fn peek_reads_the_state_without_a_clone() {
    let room = Room::default();
    room.apply(&Command::SetDuration { ms: 7 * MIN }, T0);
    assert_eq!(room.peek(|state| state.timer.duration_ms), 7 * MIN);
}

#[test]
fn the_room_records_elapsed_time_across_a_pause() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    let state = room.apply(&Command::Pause, T0 + 90_000);
    assert_eq!(state.timer.elapsed_ms, 90_000);
    assert_eq!(state.timer.run, Run::Paused);
}

#[test]
fn set_thresholds_applies_both_values() {
    let room = Room::default();
    let state = room.apply(
        &Command::SetThresholds {
            warn_ms: 2 * MIN,
            danger_ms: 30_000,
        },
        T0,
    );
    assert_eq!(state.timer.warn_ms, 2 * MIN);
    assert_eq!(state.timer.danger_ms, 30_000);
}

#[test]
fn set_on_expire_applies_the_value() {
    let room = Room::default();
    let state = room.apply(
        &Command::SetOnExpire {
            on_expire: OnExpire::HoldAtZero,
        },
        T0,
    );
    assert_eq!(state.timer.on_expire, OnExpire::HoldAtZero);
}

#[test]
fn parses_the_transport_commands() {
    assert_eq!(parse(r#"{"cmd":"start"}"#), Command::Start);
    assert_eq!(parse(r#"{"cmd":"pause"}"#), Command::Pause);
    assert_eq!(parse(r#"{"cmd":"reset"}"#), Command::Reset);
}

#[test]
fn parses_the_commands_that_carry_a_value() {
    assert_eq!(
        parse(r#"{"cmd":"set_duration","ms":900000}"#),
        Command::SetDuration { ms: 900_000 }
    );
    assert_eq!(
        parse(r#"{"cmd":"adjust","ms":-30000}"#),
        Command::Adjust { ms: -30_000 }
    );
    assert_eq!(
        parse(r#"{"cmd":"set_mode","mode":"count_up"}"#),
        Command::SetMode {
            mode: Mode::CountUp
        }
    );
}

#[test]
fn rejects_an_unknown_command() {
    assert!(serde_json::from_str::<Command>(r#"{"cmd":"explode"}"#).is_err());
}

#[test]
fn rejects_a_command_missing_its_value() {
    assert!(serde_json::from_str::<Command>(r#"{"cmd":"set_duration"}"#).is_err());
}
