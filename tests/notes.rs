//! Timed speaker notes: the shapes a rundown can carry them in, and what the
//! room keeps.

use simple_confidence_monitor::room::{
    Command, MAX_NOTES_PER_CUE, NOTE_TEXT_LIMIT, Note, Room, RoomState,
};
use simple_confidence_monitor::rundown_io::{
    notes_from_text, notes_to_text, parse_csv, parse_json,
};

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn cue_notes(state: &RoomState) -> Vec<Note> {
    state.rundown.cues[0].notes.clone()
}

fn note(at_ms: u64, text: &str) -> Note {
    Note {
        at_ms,
        text: text.to_string(),
    }
}

fn set_cues(room: &Room, json: &str) -> RoomState {
    let command: Command =
        serde_json::from_str(json).unwrap_or_else(|err| panic!("{json} should parse: {err}"));
    room.apply(&command, T0)
}

#[test]
fn a_note_list_keeps_its_times() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","duration":"30:00","notes":[
            {"at":"0:00","text":"Open with the demo"},
            {"at":"5:00","text":"Three pillars"},
            {"at":"25:00","text":"Wrap up"}
        ]}]}"#,
    );
    assert_eq!(
        cue_notes(&state),
        vec![
            note(0, "Open with the demo"),
            note(5 * MIN, "Three pillars"),
            note(25 * MIN, "Wrap up"),
        ]
    );
}

#[test]
fn a_bare_number_reads_as_minutes_like_a_duration_does() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","duration":30,"notes":[
            {"at":5,"text":"Three pillars"}
        ]}]}"#,
    );
    assert_eq!(cue_notes(&state), vec![note(5 * MIN, "Three pillars")]);
}

#[test]
fn at_ms_is_accepted_as_well_as_at() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","notes":[
            {"at_ms":90000,"text":"Ninety seconds in"}
        ]}]}"#,
    );
    assert_eq!(cue_notes(&state), vec![note(90_000, "Ninety seconds in")]);
}

// Rundowns and snapshots written before notes had times carry one string.
#[test]
fn a_plain_string_becomes_one_note_at_the_start() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","notes":"hard out at 10:20"}]}"#,
    );
    assert_eq!(cue_notes(&state), vec![note(0, "hard out at 10:20")]);
}

#[test]
fn a_missing_note_field_leaves_a_cue_without_notes() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","duration":"5:00"}]}"#,
    );
    assert!(cue_notes(&state).is_empty());
}

#[test]
fn notes_come_back_in_time_order() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","notes":[
            {"at":"20:00","text":"last"},
            {"at":"0:00","text":"first"},
            {"at":"5:00","text":"middle"}
        ]}]}"#,
    );
    let texts: Vec<String> = cue_notes(&state)
        .into_iter()
        .map(|note| note.text)
        .collect();
    assert_eq!(texts, vec!["first", "middle", "last"]);
}

#[test]
fn an_empty_note_is_dropped() {
    let room = Room::default();
    let state = set_cues(
        &room,
        r#"{"cmd":"set_cues","cues":[{"title":"Keynote","notes":[
            {"at":"0:00","text":"keep"},
            {"at":"1:00","text":"   "},
            {"at":"2:00"}
        ]}]}"#,
    );
    assert_eq!(cue_notes(&state), vec![note(0, "keep")]);
}

#[test]
fn a_cue_stops_at_the_note_ceiling() {
    let notes: Vec<String> = (0..MAX_NOTES_PER_CUE + 5)
        .map(|index| format!(r#"{{"at":{index},"text":"note {index}"}}"#))
        .collect();
    let room = Room::default();
    let state = set_cues(
        &room,
        &format!(
            r#"{{"cmd":"set_cues","cues":[{{"title":"Keynote","notes":[{}]}}]}}"#,
            notes.join(",")
        ),
    );
    assert_eq!(cue_notes(&state).len(), MAX_NOTES_PER_CUE);
}

#[test]
fn a_long_note_is_capped() {
    let room = Room::default();
    let state = room.apply(
        &Command::AddCue {
            title: Some("Keynote".into()),
            speaker: None,
            duration_ms: None,
            notes: Some(vec![note(0, &"x".repeat(NOTE_TEXT_LIMIT + 200))]),
        },
        T0,
    );
    assert_eq!(cue_notes(&state)[0].text.chars().count(), NOTE_TEXT_LIMIT);
}

#[test]
fn updating_a_cue_replaces_its_notes() {
    let room = Room::default();
    let state = room.apply(
        &Command::AddCue {
            title: Some("Keynote".into()),
            speaker: None,
            duration_ms: None,
            notes: Some(vec![note(0, "first plan")]),
        },
        T0,
    );
    let id = state.rundown.cues[0].id;
    let state = room.apply(
        &Command::UpdateCue {
            id,
            title: None,
            speaker: None,
            duration_ms: None,
            notes: Some(vec![note(0, "second plan"), note(5 * MIN, "then this")]),
        },
        T0,
    );
    assert_eq!(
        cue_notes(&state),
        vec![note(0, "second plan"), note(5 * MIN, "then this")]
    );
}

#[test]
fn leaving_notes_out_of_an_update_keeps_them() {
    let room = Room::default();
    let state = room.apply(
        &Command::AddCue {
            title: Some("Keynote".into()),
            speaker: None,
            duration_ms: None,
            notes: Some(vec![note(0, "keep me")]),
        },
        T0,
    );
    let id = state.rundown.cues[0].id;
    let state = room.apply(
        &Command::UpdateCue {
            id,
            title: Some("Keynote II".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    assert_eq!(cue_notes(&state), vec![note(0, "keep me")]);
}

#[test]
fn a_note_that_is_not_a_note_says_which_one() {
    let error = parse_json(
        r#"{"cues":[{"title":"Keynote","notes":[{"at":"0:00","text":"fine"},{"at":"nope","text":"bad"}]}]}"#,
    )
    .unwrap_err();
    assert!(error.contains("note 2"), "got {error}");
}

#[test]
fn the_text_form_reads_a_time_prefix() {
    assert_eq!(
        notes_from_text("0:00 Open with the demo\n5:00 Three pillars"),
        vec![
            note(0, "Open with the demo"),
            note(5 * MIN, "Three pillars")
        ]
    );
}

#[test]
fn the_text_form_continues_a_note_on_an_indented_line() {
    assert_eq!(
        notes_from_text("5:00 Three pillars,\n  then the customer slide"),
        vec![note(5 * MIN, "Three pillars,\nthen the customer slide")]
    );
}

// A note is prose. "3 things to cover" is not a cue point three minutes in.
#[test]
fn the_text_form_leaves_a_bare_number_as_text() {
    assert_eq!(
        notes_from_text("3 things to cover"),
        vec![note(0, "3 things to cover")]
    );
}

#[test]
fn the_text_form_round_trips() {
    let notes = vec![
        note(0, "Open with the demo"),
        note(5 * MIN, "Three pillars,\nthen the customer slide"),
        note(25 * MIN, "Wrap up"),
    ];
    assert_eq!(notes_from_text(&notes_to_text(&notes)), notes);
}

#[test]
fn a_csv_note_column_carries_the_times() {
    let csv = "title,speaker,duration,notes\nKeynote,Alice,30:00,\"0:00 Open with the demo\n5:00 Three pillars\"\n";
    let cues = parse_csv(csv).unwrap();
    assert_eq!(
        cues[0].notes,
        vec![
            note(0, "Open with the demo"),
            note(5 * MIN, "Three pillars")
        ]
    );
}

#[test]
fn an_import_over_the_notes_budget_is_refused() {
    let cue = format!(
        r#"{{"title":"Keynote","notes":[{{"at":"0:00","text":"{}"}}]}}"#,
        "x".repeat(NOTE_TEXT_LIMIT)
    );
    let cues: Vec<String> = (0..400).map(|_| cue.clone()).collect();
    let error = parse_json(&format!(r#"{{"cues":[{}]}}"#, cues.join(","))).unwrap_err();
    assert!(error.contains("characters of notes"), "got {error}");
}
