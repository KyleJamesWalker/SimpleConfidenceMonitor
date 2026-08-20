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
