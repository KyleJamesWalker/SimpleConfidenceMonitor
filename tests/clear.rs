use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::room::{Command, Room, RoomName, Tone};
use simple_confidence_monitor::timer::Run;

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

/// A room with something in every part of its state.
fn busy_room() -> Room {
    let room = Room::default();
    room.apply(&Command::SetDuration { ms: 42 * MIN }, T0);
    room.apply(&Command::Start, T0);
    room.apply(
        &Command::Message {
            text: Some("Wrap up".into()),
            tone: Some(Tone::Alert),
            visible: Some(true),
        },
        T0,
    );
    room.apply(
        &Command::Display {
            title: Some("Keynote".into()),
            next_up: Some("Panel".into()),
            show_clock: Some(false),
            clock_24h: None,
            show_progress: None,
            mirror: Some(true),
            scale: Some(150),
            chime: Some(true),
        },
        T0,
    );
    room.apply(
        &Command::AddCue {
            title: Some("Welcome".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    room.apply(
        &Command::AuxSet {
            label: Some("Break".into()),
            visible: Some(true),
        },
        T0,
    );
    room.apply(&Command::SetPresets { presets: vec![] }, T0);
    room
}

#[test]
fn clearing_a_room_returns_every_part_to_its_default() {
    let room = busy_room();
    let before = room.snapshot();
    let state = room.apply(&Command::ClearRoom, T0 + MIN);

    assert_eq!(state.timer.duration_ms, 900_000);
    assert_eq!(state.timer.run, Run::Stopped);
    assert_eq!(state.message.text, "");
    assert!(!state.message.visible);
    assert_eq!(state.display.title, "");
    assert_eq!(state.display.scale, 100);
    assert!(!state.display.mirror);
    assert!(!state.display.chime);
    assert!(state.rundown.cues.is_empty());
    assert_eq!(state.rundown.active, None);
    assert!(!state.aux.visible);
    assert_eq!(state.aux.label, "");
    assert!(!state.presets.is_empty(), "the default presets come back");
    assert!(state.rev > before.rev, "clients need to see the change");
}

#[test]
fn clearing_a_fresh_room_changes_nothing() {
    let room = Room::default();
    assert_eq!(room.apply(&Command::ClearRoom, T0).rev, 0);
}

#[test]
fn parses_the_clear_command() {
    let command: Command = serde_json::from_str(r#"{"cmd":"clear_room"}"#).unwrap();
    assert_eq!(command, Command::ClearRoom);
}

#[test]
fn a_removed_room_leaves_the_registry() {
    let hub = Hub::new();
    let name = RoomName::parse("keynote").unwrap();
    hub.get_or_create(&name);
    assert_eq!(hub.room_count(), 1);

    assert!(hub.remove(&name));
    assert_eq!(hub.room_count(), 0);
    assert!(hub.get(&name).is_none());
}

#[test]
fn removing_a_room_that_is_not_there_reports_it() {
    let hub = Hub::new();
    assert!(!hub.remove(&RoomName::parse("ghost").unwrap()));
}
