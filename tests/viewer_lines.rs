use simple_confidence_monitor::room::{Command, Room};

const T0: u64 = 1_700_000_000_000;

fn display(show_speaker: Option<bool>, show_notes: Option<bool>) -> Command {
    Command::Display {
        title: None,
        next_up: None,
        show_clock: None,
        clock_24h: None,
        show_progress: None,
        mirror: None,
        scale: None,
        chime: None,
        show_speaker,
        show_notes,
    }
}

#[test]
fn a_new_room_shows_the_speaker_and_hides_the_notes() {
    let display = Room::default().snapshot().display;
    assert!(display.show_speaker, "a speaker name belongs on stage");
    assert!(
        !display.show_notes,
        "a note may be for the crew, not the room"
    );
}

#[test]
fn the_display_command_toggles_both_lines() {
    let room = Room::default();
    let state = room.apply(&display(Some(false), Some(true)), T0);
    assert!(!state.display.show_speaker);
    assert!(state.display.show_notes);
}

#[test]
fn toggling_one_line_leaves_the_other_alone() {
    let room = Room::default();
    room.apply(&display(Some(false), Some(true)), T0);
    let state = room.apply(&display(None, Some(false)), T0);
    assert!(!state.display.show_speaker);
    assert!(!state.display.show_notes);
}

#[test]
fn repeating_the_command_does_not_bump_rev() {
    let room = Room::default();
    let first = room.apply(&display(Some(false), Some(true)), T0);
    assert_eq!(
        room.apply(&display(Some(false), Some(true)), T0).rev,
        first.rev
    );
}

#[test]
fn parses_the_line_fields() {
    let command: Command =
        serde_json::from_str(r#"{"cmd":"display","show_speaker":false,"show_notes":true}"#)
            .unwrap();
    assert_eq!(command, display(Some(false), Some(true)));
}
