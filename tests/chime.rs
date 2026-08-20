use simple_confidence_monitor::room::{Command, Room};

const T0: u64 = 1_700_000_000_000;

fn display_chime(on: bool) -> Command {
    Command::Display {
        title: None,
        next_up: None,
        show_clock: None,
        clock_24h: None,
        show_progress: None,
        mirror: None,
        scale: None,
        chime: Some(on),
        show_speaker: None,
        show_notes: None,
    }
}

#[test]
fn a_new_room_keeps_the_chime_off() {
    assert!(!Room::default().snapshot().display.chime);
}

#[test]
fn the_display_command_turns_the_chime_on_and_off() {
    let room = Room::default();
    assert!(room.apply(&display_chime(true), T0).display.chime);
    assert!(!room.apply(&display_chime(false), T0).display.chime);
}

#[test]
fn setting_the_chime_twice_does_not_bump_rev() {
    let room = Room::default();
    let first = room.apply(&display_chime(true), T0);
    assert_eq!(room.apply(&display_chime(true), T0).rev, first.rev);
}

#[test]
fn a_display_command_without_a_chime_field_leaves_it_alone() {
    let room = Room::default();
    room.apply(&display_chime(true), T0);
    let state = room.apply(
        &Command::Display {
            title: Some("Keynote".into()),
            next_up: None,
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
    assert!(state.display.chime);
}

#[test]
fn parses_a_chime_command() {
    let command: Command = serde_json::from_str(r#"{"cmd":"display","chime":true}"#).unwrap();
    assert_eq!(command, display_chime(true));
}
