use simple_confidence_monitor::room::{Command, DEFAULT_AUX_MS, Room};
use simple_confidence_monitor::timer::Run;

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn parse(json: &str) -> Command {
    serde_json::from_str(json).expect("command should parse")
}

fn aux_set(label: Option<&str>, visible: Option<bool>) -> Command {
    Command::AuxSet {
        label: label.map(str::to_string),
        visible,
    }
}

#[test]
fn a_new_room_hides_an_idle_aux_timer() {
    let aux = Room::default().snapshot().aux;
    assert!(!aux.visible);
    assert_eq!(aux.label, "");
    assert_eq!(aux.timer.run, Run::Stopped);
    assert_eq!(aux.timer.duration_ms, DEFAULT_AUX_MS);
}

#[test]
fn the_aux_timer_runs_on_its_own_clock() {
    let room = Room::default();
    room.apply(&Command::AuxSetDuration { ms: 10 * MIN }, T0);
    room.apply(&Command::AuxStart, T0);
    let state = room.apply(&Command::AuxPause, T0 + 90_000);
    assert_eq!(state.aux.timer.elapsed_ms, 90_000);
    assert_eq!(state.aux.timer.run, Run::Paused);
}

#[test]
fn the_aux_timer_never_moves_the_main_timer() {
    let room = Room::default();
    room.apply(&Command::AuxStart, T0);
    room.apply(&Command::AuxSetDuration { ms: MIN }, T0);
    room.apply(&Command::AuxAdjust { ms: 30_000 }, T0);
    let state = room.apply(&Command::AuxReset, T0 + MIN);
    assert_eq!(state.timer.run, Run::Stopped);
    assert_eq!(state.timer.elapsed_ms, 0);
    assert_eq!(state.timer.duration_ms, 900_000);
}

#[test]
fn the_main_timer_never_moves_the_aux_timer() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    room.apply(&Command::SetDuration { ms: MIN }, T0);
    let state = room.apply(&Command::Reset, T0 + MIN);
    assert_eq!(state.aux.timer.run, Run::Stopped);
    assert_eq!(state.aux.timer.duration_ms, DEFAULT_AUX_MS);
}

#[test]
fn both_timers_run_at_the_same_time() {
    let room = Room::default();
    room.apply(&Command::Start, T0);
    room.apply(&Command::AuxStart, T0 + 30_000);
    let state = room.snapshot();
    assert_eq!(state.timer.run, Run::Running { since_ms: T0 });
    assert_eq!(
        state.aux.timer.run,
        Run::Running {
            since_ms: T0 + 30_000
        }
    );
    assert_eq!(state.timer.elapsed_at(T0 + 90_000), 90_000);
    assert_eq!(state.aux.timer.elapsed_at(T0 + 90_000), 60_000);
}

#[test]
fn aux_reset_returns_to_the_full_duration() {
    let room = Room::default();
    room.apply(&Command::AuxStart, T0);
    let state = room.apply(&Command::AuxReset, T0 + MIN);
    assert_eq!(state.aux.timer.run, Run::Stopped);
    assert_eq!(state.aux.timer.elapsed_ms, 0);
}

#[test]
fn aux_adjust_never_takes_the_target_below_zero() {
    let room = Room::default();
    room.apply(&Command::AuxSetDuration { ms: 10_000 }, T0);
    let state = room.apply(&Command::AuxAdjust { ms: -60_000 }, T0);
    assert_eq!(state.aux.timer.duration_ms, 0);
}

#[test]
fn aux_set_changes_only_the_fields_it_names() {
    let room = Room::default();
    room.apply(&aux_set(Some("Break"), Some(true)), T0);
    let state = room.apply(&aux_set(None, Some(false)), T0);
    assert_eq!(state.aux.label, "Break");
    assert!(!state.aux.visible);
}

#[test]
fn repeating_an_aux_command_does_not_bump_rev() {
    let room = Room::default();
    let first = room.apply(&aux_set(Some("Break"), Some(true)), T0);
    assert_eq!(
        room.apply(&aux_set(Some("Break"), Some(true)), T0).rev,
        first.rev
    );
    room.apply(&Command::AuxStart, T0);
    let running = room.snapshot();
    assert_eq!(room.apply(&Command::AuxStart, T0 + MIN).rev, running.rev);
}

#[test]
fn parses_the_aux_commands() {
    assert_eq!(parse(r#"{"cmd":"aux_start"}"#), Command::AuxStart);
    assert_eq!(parse(r#"{"cmd":"aux_pause"}"#), Command::AuxPause);
    assert_eq!(parse(r#"{"cmd":"aux_reset"}"#), Command::AuxReset);
    assert_eq!(
        parse(r#"{"cmd":"aux_set_duration","ms":600000}"#),
        Command::AuxSetDuration { ms: 600_000 }
    );
    assert_eq!(
        parse(r#"{"cmd":"aux_adjust","ms":-30000}"#),
        Command::AuxAdjust { ms: -30_000 }
    );
    assert_eq!(
        parse(r#"{"cmd":"aux_set","label":"Break","visible":true}"#),
        aux_set(Some("Break"), Some(true))
    );
    assert_eq!(parse(r#"{"cmd":"aux_set"}"#), aux_set(None, None));
}
