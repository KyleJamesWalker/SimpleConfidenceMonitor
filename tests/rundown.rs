use simple_confidence_monitor::room::{Command, CueDraft, Room, RoomState};
use simple_confidence_monitor::timer::Run;

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn parse(json: &str) -> Command {
    serde_json::from_str(json).expect("command should parse")
}

fn add(title: &str, minutes: u64) -> Command {
    Command::AddCue {
        title: Some(title.to_string()),
        speaker: None,
        duration_ms: Some(minutes * MIN),
        notes: None,
    }
}

/// Builds a room holding three cues, and returns their ids in order.
fn with_cues(room: &Room) -> Vec<u64> {
    for (title, minutes) in [("Welcome", 5), ("Keynote", 30), ("Panel", 20)] {
        room.apply(&add(title, minutes), T0);
    }
    ids(&room.snapshot())
}

fn ids(state: &RoomState) -> Vec<u64> {
    state.rundown.cues.iter().map(|cue| cue.id).collect()
}

fn titles(state: &RoomState) -> Vec<String> {
    state
        .rundown
        .cues
        .iter()
        .map(|cue| cue.title.clone())
        .collect()
}

#[test]
fn a_new_room_holds_an_empty_rundown() {
    let state = Room::default().snapshot();
    assert!(state.rundown.cues.is_empty());
    assert_eq!(state.rundown.active, None);
    assert!(!state.rundown.auto_advance);
}

#[test]
fn adding_a_cue_appends_it_with_a_fresh_id() {
    let room = Room::default();
    let state = room.apply(&add("Welcome", 5), T0);
    assert_eq!(state.rundown.cues.len(), 1);
    assert_eq!(state.rundown.cues[0].title, "Welcome");
    assert_eq!(state.rundown.cues[0].duration_ms, 5 * MIN);

    let state = room.apply(&add("Keynote", 30), T0);
    assert_eq!(titles(&state), vec!["Welcome", "Keynote"]);
    assert_ne!(state.rundown.cues[0].id, state.rundown.cues[1].id);
}

#[test]
fn an_id_is_never_reused_after_a_removal() {
    let room = Room::default();
    let first = with_cues(&room);
    room.apply(&Command::RemoveCue { id: first[2] }, T0);
    let state = room.apply(&add("Closing", 10), T0);
    let last = state.rundown.cues.last().unwrap();
    assert!(!first.contains(&last.id), "reused id {}", last.id);
}

#[test]
fn updating_a_cue_changes_only_the_fields_it_names() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(
        &Command::UpdateCue {
            id: ids[1],
            title: None,
            speaker: Some("Alice".into()),
            duration_ms: Some(45 * MIN),
            notes: None,
        },
        T0,
    );
    let cue = &state.rundown.cues[1];
    assert_eq!(cue.title, "Keynote");
    assert_eq!(cue.speaker, "Alice");
    assert_eq!(cue.duration_ms, 45 * MIN);
}

#[test]
fn updating_a_missing_cue_changes_nothing() {
    let room = Room::default();
    with_cues(&room);
    let before = room.snapshot();
    let state = room.apply(
        &Command::UpdateCue {
            id: 9999,
            title: Some("Ghost".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    assert_eq!(state.rev, before.rev);
    assert_eq!(titles(&state), titles(&before));
}

#[test]
fn removing_a_cue_drops_it_and_keeps_the_order() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::RemoveCue { id: ids[1] }, T0);
    assert_eq!(titles(&state), vec!["Welcome", "Panel"]);
}

#[test]
fn removing_the_active_cue_clears_the_active_marker() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[1] }, T0);
    let state = room.apply(&Command::RemoveCue { id: ids[1] }, T0);
    assert_eq!(state.rundown.active, None);
}

#[test]
fn moving_a_cue_reorders_the_list() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::MoveCue { id: ids[2], to: 0 }, T0);
    assert_eq!(titles(&state), vec!["Panel", "Welcome", "Keynote"]);
}

#[test]
fn moving_past_the_end_puts_the_cue_last() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::MoveCue { id: ids[0], to: 99 }, T0);
    assert_eq!(titles(&state), vec!["Keynote", "Panel", "Welcome"]);
}

#[test]
fn loading_a_cue_sets_the_timer_and_the_screen() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::LoadCue { id: ids[1] }, T0);
    assert_eq!(state.rundown.active, Some(ids[1]));
    assert_eq!(state.timer.duration_ms, 30 * MIN);
    assert_eq!(state.timer.run, Run::Stopped);
    assert_eq!(state.timer.elapsed_ms, 0);
    assert_eq!(state.display.title, "Keynote");
    assert_eq!(state.display.next_up, "Panel");
}

#[test]
fn loading_the_last_cue_leaves_no_next_up() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::LoadCue { id: ids[2] }, T0);
    assert_eq!(state.display.next_up, "");
}

#[test]
fn loading_a_cue_while_running_restarts_the_clock() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    room.apply(&Command::Start, T0);
    let state = room.apply(&Command::LoadCue { id: ids[1] }, T0 + 2 * MIN);
    assert_eq!(state.timer.run, Run::Stopped);
    assert_eq!(state.timer.elapsed_ms, 0);
}

#[test]
fn next_cue_walks_forward_and_stops_at_the_end() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    assert_eq!(
        room.apply(&Command::NextCue, T0).rundown.active,
        Some(ids[1])
    );
    assert_eq!(
        room.apply(&Command::NextCue, T0).rundown.active,
        Some(ids[2])
    );
    let state = room.apply(&Command::NextCue, T0);
    assert_eq!(state.rundown.active, Some(ids[2]));
}

#[test]
fn previous_cue_walks_back_and_stops_at_the_start() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[1] }, T0);
    assert_eq!(
        room.apply(&Command::PrevCue, T0).rundown.active,
        Some(ids[0])
    );
    let state = room.apply(&Command::PrevCue, T0);
    assert_eq!(state.rundown.active, Some(ids[0]));
}

#[test]
fn next_cue_with_nothing_active_loads_the_first() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::NextCue, T0);
    assert_eq!(state.rundown.active, Some(ids[0]));
}

#[test]
fn next_cue_on_an_empty_rundown_changes_nothing() {
    let room = Room::default();
    let state = room.apply(&Command::NextCue, T0);
    assert_eq!(state.rev, 0);
    assert_eq!(state.rundown.active, None);
}

#[test]
fn auto_advance_toggles() {
    let room = Room::default();
    assert!(
        room.apply(&Command::SetAutoAdvance { on: true }, T0)
            .rundown
            .auto_advance
    );
    assert_eq!(room.apply(&Command::SetAutoAdvance { on: true }, T0).rev, 1);
}

#[test]
fn parses_the_rundown_commands() {
    assert_eq!(
        parse(r#"{"cmd":"add_cue","title":"Welcome","duration_ms":300000}"#),
        Command::AddCue {
            title: Some("Welcome".into()),
            speaker: None,
            duration_ms: Some(300_000),
            notes: None
        }
    );
    assert_eq!(parse(r#"{"cmd":"next_cue"}"#), Command::NextCue);
    assert_eq!(parse(r#"{"cmd":"prev_cue"}"#), Command::PrevCue);
    assert_eq!(
        parse(r#"{"cmd":"load_cue","id":3}"#),
        Command::LoadCue { id: 3 }
    );
    assert_eq!(
        parse(r#"{"cmd":"remove_cue","id":3}"#),
        Command::RemoveCue { id: 3 }
    );
    assert_eq!(
        parse(r#"{"cmd":"move_cue","id":3,"to":0}"#),
        Command::MoveCue { id: 3, to: 0 }
    );
    assert_eq!(
        parse(r#"{"cmd":"set_auto_advance","on":true}"#),
        Command::SetAutoAdvance { on: true }
    );
}

#[test]
fn editing_the_active_cue_leaves_a_running_timer_alone() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[1] }, T0);
    room.apply(&Command::Start, T0);

    let state = room.apply(
        &Command::UpdateCue {
            id: ids[1],
            title: None,
            speaker: None,
            duration_ms: None,
            notes: Some("remember the mic".into()),
        },
        T0 + 5 * MIN,
    );

    assert_eq!(
        state.timer.run,
        Run::Running { since_ms: T0 },
        "a note edit must not stop the clock on a talk in progress"
    );
    assert_eq!(state.timer.elapsed_at(T0 + 5 * MIN), 5 * MIN);
    assert_eq!(state.rundown.cues[1].notes, "remember the mic");
}

#[test]
fn editing_the_active_cue_title_still_updates_the_screen() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    let state = room.apply(
        &Command::UpdateCue {
            id: ids[0],
            title: Some("Doors".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    assert_eq!(state.display.title, "Doors");
}

#[test]
fn changing_the_active_cue_duration_retargets_a_running_timer() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[1] }, T0);
    room.apply(&Command::Start, T0);
    let state = room.apply(
        &Command::UpdateCue {
            id: ids[1],
            title: None,
            speaker: None,
            duration_ms: Some(45 * MIN),
            notes: None,
        },
        T0 + MIN,
    );
    assert_eq!(state.timer.duration_ms, 45 * MIN);
    assert_eq!(state.timer.run, Run::Running { since_ms: T0 });
}

#[test]
fn next_and_start_loads_the_following_cue_and_runs_it() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);

    let state = room.apply(&Command::NextAndStart, T0 + MIN);
    assert_eq!(state.rundown.active, Some(ids[1]));
    assert_eq!(state.timer.run, Run::Running { since_ms: T0 + MIN });
    assert_eq!(state.timer.duration_ms, 30 * MIN);
    assert_eq!(state.timer.elapsed_ms, 0);
}

#[test]
fn next_and_start_with_nothing_loaded_runs_the_first_cue() {
    let room = Room::default();
    let ids = with_cues(&room);
    let state = room.apply(&Command::NextAndStart, T0);
    assert_eq!(state.rundown.active, Some(ids[0]));
    assert!(state.timer.is_running());
}

#[test]
fn next_and_start_on_the_last_cue_leaves_it_alone() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[2] }, T0);
    room.apply(&Command::Start, T0);
    let before = room.snapshot();

    let state = room.apply(&Command::NextAndStart, T0 + MIN);
    assert_eq!(state.rundown.active, Some(ids[2]), "nowhere to go");
    assert_eq!(state.rev, before.rev, "a no-op must not wake the screens");
    assert_eq!(
        state.timer.run,
        Run::Running { since_ms: T0 },
        "the running talk keeps its clock"
    );
}

#[test]
fn next_and_start_on_an_empty_rundown_changes_nothing() {
    let room = Room::default();
    let state = room.apply(&Command::NextAndStart, T0);
    assert_eq!(state.rev, 0);
    assert!(!state.timer.is_running());
}

#[test]
fn next_and_start_lands_in_one_revision() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    let before = room.snapshot().rev;
    let state = room.apply(&Command::NextAndStart, T0);
    assert_eq!(state.rev, before + 1, "one operator action, one revision");
}

#[test]
fn parses_next_and_start() {
    assert_eq!(parse(r#"{"cmd":"next_and_start"}"#), Command::NextAndStart);
}

#[test]
fn deleting_the_active_cue_clears_the_screen() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    let state = room.apply(&Command::RemoveCue { id: ids[0] }, T0);
    assert_eq!(state.display.title, "", "the title came from that cue");
    assert_eq!(state.display.next_up, "");
    assert_eq!(state.rundown.active, None);
}

#[test]
fn deleting_every_cue_leaves_a_bare_timer() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    for id in &ids {
        room.apply(&Command::RemoveCue { id: *id }, T0);
    }
    let state = room.snapshot();
    assert!(state.rundown.cues.is_empty());
    assert_eq!(state.display.title, "");
    assert_eq!(state.display.next_up, "");
}

#[test]
fn deleting_the_cue_that_was_next_moves_the_next_up_line() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    assert_eq!(room.snapshot().display.next_up, "Keynote");

    let state = room.apply(&Command::RemoveCue { id: ids[1] }, T0);
    assert_eq!(state.display.next_up, "Panel", "the line follows the list");
    assert_eq!(
        state.display.title, "Welcome",
        "the active cue is untouched"
    );
}

#[test]
fn adding_a_cue_after_the_last_one_fills_the_next_up_line() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[2] }, T0);
    assert_eq!(room.snapshot().display.next_up, "");

    let state = room.apply(&add("Closing", 10), T0);
    assert_eq!(state.display.next_up, "Closing");
}

#[test]
fn renaming_the_following_cue_updates_the_next_up_line() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    let state = room.apply(
        &Command::UpdateCue {
            id: ids[1],
            title: Some("Keynote: rescheduled".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    assert_eq!(state.display.next_up, "Keynote: rescheduled");
}

#[test]
fn reordering_the_rundown_updates_the_next_up_line() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    let state = room.apply(&Command::MoveCue { id: ids[2], to: 1 }, T0);
    assert_eq!(state.display.next_up, "Panel");
}

#[test]
fn replacing_the_rundown_clears_a_screen_whose_cue_is_gone() {
    let room = Room::default();
    let ids = with_cues(&room);
    room.apply(&Command::LoadCue { id: ids[0] }, T0);
    let state = room.apply(
        &Command::SetCues {
            cues: vec![CueDraft {
                title: "Something else".into(),
                speaker: String::new(),
                duration_ms: 5 * MIN,
                notes: String::new(),
            }],
        },
        T0,
    );
    assert_eq!(state.rundown.active, None);
    assert_eq!(state.display.title, "");
    assert_eq!(state.display.next_up, "");
}

#[test]
fn a_typed_title_survives_a_rundown_edit_when_no_cue_is_loaded() {
    let room = Room::default();
    room.apply(
        &Command::Display {
            title: Some("Morning session".into()),
            next_up: Some("Lunch".into()),
            show_clock: None,
            clock_24h: None,
            show_progress: None,
            mirror: None,
            scale: None,
            chime: None,
            show_speaker: None,
            show_notes: None,
        },
        T0,
    );
    let state = room.apply(&add("Welcome", 5), T0);
    assert_eq!(
        state.display.title, "Morning session",
        "nobody asked to change it"
    );
    assert_eq!(state.display.next_up, "Lunch");
}
