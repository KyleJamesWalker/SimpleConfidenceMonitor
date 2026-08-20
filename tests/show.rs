use simple_confidence_monitor::room::{Command, MAX_SCALE, MIN_SCALE, Room, Tone};

const T0: u64 = 1_700_000_000_000;

fn parse(json: &str) -> Command {
    serde_json::from_str(json).expect("command should parse")
}

fn message(text: &str) -> Command {
    Command::Message {
        text: Some(text.to_string()),
        tone: None,
        visible: Some(true),
    }
}

#[test]
fn a_new_room_shows_no_message() {
    let state = Room::default().snapshot();
    assert_eq!(state.message.text, "");
    assert!(!state.message.visible);
    assert_eq!(state.message.tone, Tone::Neutral);
}

#[test]
fn a_new_room_shows_the_clock_and_the_progress_bar() {
    let display = Room::default().snapshot().display;
    assert!(display.show_clock);
    assert!(display.clock_24h);
    assert!(display.show_progress);
    assert!(!display.blackout);
    assert!(!display.mirror);
    assert_eq!(display.scale, 100);
}

#[test]
fn a_message_command_sets_the_text_and_shows_it() {
    let room = Room::default();
    let state = room.apply(&message("Wrap up"), T0);
    assert_eq!(state.message.text, "Wrap up");
    assert!(state.message.visible);
    assert_eq!(state.rev, 1);
}

#[test]
fn a_message_command_sets_the_tone() {
    let room = Room::default();
    let state = room.apply(
        &Command::Message {
            text: Some("Two minutes".into()),
            tone: Some(Tone::Warn),
            visible: Some(true),
        },
        T0,
    );
    assert_eq!(state.message.tone, Tone::Warn);
}

#[test]
fn hiding_a_message_keeps_the_text() {
    let room = Room::default();
    room.apply(&message("Wrap up"), T0);
    let state = room.apply(
        &Command::Message {
            text: None,
            tone: None,
            visible: Some(false),
        },
        T0,
    );
    assert_eq!(state.message.text, "Wrap up");
    assert!(!state.message.visible);
}

#[test]
fn repeating_a_message_does_not_bump_rev() {
    let room = Room::default();
    room.apply(&message("Wrap up"), T0);
    assert_eq!(room.apply(&message("Wrap up"), T0).rev, 1);
}

#[test]
fn flash_stamps_the_time_and_always_counts_as_a_change() {
    let room = Room::default();
    let first = room.apply(&Command::Flash, T0);
    assert_eq!(first.display.flash_at, T0);
    let second = room.apply(&Command::Flash, T0 + 5_000);
    assert_eq!(second.display.flash_at, T0 + 5_000);
    assert_eq!(second.rev, 2);
}

#[test]
fn blackout_toggles_and_ignores_a_repeat() {
    let room = Room::default();
    assert!(
        room.apply(&Command::Blackout { on: true }, T0)
            .display
            .blackout
    );
    assert_eq!(room.apply(&Command::Blackout { on: true }, T0).rev, 1);
    assert!(
        !room
            .apply(&Command::Blackout { on: false }, T0)
            .display
            .blackout
    );
}

#[test]
fn a_display_command_updates_only_the_fields_it_names() {
    let room = Room::default();
    room.apply(
        &Command::Display {
            title: Some("Keynote".into()),
            next_up: Some("Panel".into()),
            show_clock: None,
            clock_24h: None,
            show_progress: None,
            mirror: None,
            scale: None,
        },
        T0,
    );
    let state = room.apply(
        &Command::Display {
            title: None,
            next_up: None,
            show_clock: Some(false),
            clock_24h: None,
            show_progress: None,
            mirror: None,
            scale: None,
        },
        T0,
    );
    assert_eq!(state.display.title, "Keynote");
    assert_eq!(state.display.next_up, "Panel");
    assert!(!state.display.show_clock);
}

#[test]
fn scale_clamps_to_the_supported_range() {
    let room = Room::default();
    let set = |scale| Command::Display {
        title: None,
        next_up: None,
        show_clock: None,
        clock_24h: None,
        show_progress: None,
        mirror: None,
        scale: Some(scale),
    };
    assert_eq!(room.apply(&set(10), T0).display.scale, MIN_SCALE);
    assert_eq!(room.apply(&set(255), T0).display.scale, MAX_SCALE);
    assert_eq!(room.apply(&set(150), T0).display.scale, 150);
}

#[test]
fn parses_a_partial_message_command() {
    assert_eq!(
        parse(r#"{"cmd":"message","visible":false}"#),
        Command::Message {
            text: None,
            tone: None,
            visible: Some(false)
        }
    );
}

#[test]
fn parses_the_show_commands_from_the_spec() {
    assert_eq!(parse(r#"{"cmd":"flash"}"#), Command::Flash);
    assert_eq!(
        parse(r#"{"cmd":"blackout","on":true}"#),
        Command::Blackout { on: true }
    );
    assert_eq!(
        parse(r#"{"cmd":"message","text":"Wrap up","tone":"warn","visible":true}"#),
        Command::Message {
            text: Some("Wrap up".into()),
            tone: Some(Tone::Warn),
            visible: Some(true)
        }
    );
    assert_eq!(
        parse(r#"{"cmd":"display","title":"Keynote","next_up":"Panel: Q&A"}"#),
        Command::Display {
            title: Some("Keynote".into()),
            next_up: Some("Panel: Q&A".into()),
            show_clock: None,
            clock_24h: None,
            show_progress: None,
            mirror: None,
            scale: None
        }
    );
}

#[test]
fn rejects_an_unknown_tone() {
    assert!(serde_json::from_str::<Command>(r#"{"cmd":"message","tone":"loud"}"#).is_err());
}
