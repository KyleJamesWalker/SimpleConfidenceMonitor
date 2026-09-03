use simple_confidence_monitor::room::{
    Command, CueDraft, LINE_TEXT_LIMIT, MAX_CUES, MESSAGE_TEXT_LIMIT, NOTES_TEXT_LIMIT, Room,
};
use simple_confidence_monitor::rundown_io::{parse_csv, parse_json};

const T0: u64 = 1_700_000_000_000;

fn long(count: usize) -> String {
    "x".repeat(count)
}

fn draft(title: &str) -> CueDraft {
    CueDraft {
        title: title.to_string(),
        speaker: String::new(),
        duration_ms: 60_000,
        notes: String::new(),
    }
}

#[test]
fn a_long_message_is_capped() {
    let room = Room::default();
    let state = room.apply(
        &Command::Message {
            text: Some(long(MESSAGE_TEXT_LIMIT + 500)),
            tone: None,
            visible: Some(true),
        },
        T0,
    );
    assert_eq!(state.message.text.chars().count(), MESSAGE_TEXT_LIMIT);
}

#[test]
fn a_long_title_and_next_up_are_capped() {
    let room = Room::default();
    let state = room.apply(
        &Command::Display {
            title: Some(long(LINE_TEXT_LIMIT + 40)),
            next_up: Some(long(LINE_TEXT_LIMIT + 40)),
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
    assert_eq!(state.display.title.chars().count(), LINE_TEXT_LIMIT);
    assert_eq!(state.display.next_up.chars().count(), LINE_TEXT_LIMIT);
}

#[test]
fn a_long_cue_field_is_capped_on_the_way_in() {
    let room = Room::default();
    let state = room.apply(
        &Command::AddCue {
            title: Some(long(LINE_TEXT_LIMIT + 10)),
            speaker: Some(long(LINE_TEXT_LIMIT + 10)),
            duration_ms: None,
            notes: Some(long(NOTES_TEXT_LIMIT + 10)),
        },
        T0,
    );
    let cue = &state.rundown.cues[0];
    assert_eq!(cue.title.chars().count(), LINE_TEXT_LIMIT);
    assert_eq!(cue.speaker.chars().count(), LINE_TEXT_LIMIT);
    assert_eq!(cue.notes.chars().count(), NOTES_TEXT_LIMIT);
}

#[test]
fn a_long_cue_field_is_capped_on_an_edit() {
    let room = Room::default();
    let id = room
        .apply(
            &Command::AddCue {
                title: Some("Keynote".into()),
                speaker: None,
                duration_ms: None,
                notes: None,
            },
            T0,
        )
        .rundown
        .cues[0]
        .id;
    let state = room.apply(
        &Command::UpdateCue {
            id,
            title: Some(long(LINE_TEXT_LIMIT + 10)),
            speaker: None,
            duration_ms: None,
            notes: Some(long(NOTES_TEXT_LIMIT + 10)),
        },
        T0,
    );
    assert_eq!(state.rundown.cues[0].title.chars().count(), LINE_TEXT_LIMIT);
    assert_eq!(
        state.rundown.cues[0].notes.chars().count(),
        NOTES_TEXT_LIMIT
    );
}

#[test]
fn the_rundown_stops_at_the_cue_ceiling() {
    let room = Room::default();
    for index in 0..MAX_CUES + 5 {
        room.apply(
            &Command::AddCue {
                title: Some(format!("Cue {index}")),
                speaker: None,
                duration_ms: None,
                notes: None,
            },
            T0,
        );
    }
    assert_eq!(room.snapshot().rundown.cues.len(), MAX_CUES);
}

#[test]
fn set_cues_stops_at_the_cue_ceiling() {
    let room = Room::default();
    let cues: Vec<CueDraft> = (0..MAX_CUES + 20)
        .map(|index| draft(&format!("Cue {index}")))
        .collect();
    let state = room.apply(&Command::SetCues { cues }, T0);
    assert_eq!(state.rundown.cues.len(), MAX_CUES);
}

#[test]
fn a_csv_import_over_the_ceiling_is_refused() {
    let mut body = String::from("title,speaker,duration,notes\n");
    for index in 0..MAX_CUES + 1 {
        body.push_str(&format!("Cue {index},Alice,5:00,\n"));
    }
    let err = parse_csv(&body).expect_err("an oversized import must be refused");
    assert!(err.contains("at most"), "{err}");
}

#[test]
fn a_json_import_over_the_ceiling_is_refused() {
    let cues: Vec<String> = (0..MAX_CUES + 1)
        .map(|index| format!(r#"{{"title":"Cue {index}","duration":"5:00"}}"#))
        .collect();
    let body = format!(r#"{{"cues":[{}]}}"#, cues.join(","));
    let err = parse_json(&body).expect_err("an oversized import must be refused");
    assert!(err.contains("at most"), "{err}");
}

#[test]
fn an_import_at_the_ceiling_still_lands() {
    let cues: Vec<String> = (0..MAX_CUES)
        .map(|index| format!(r#"{{"title":"Cue {index}","duration":"5:00"}}"#))
        .collect();
    let body = format!(r#"{{"cues":[{}]}}"#, cues.join(","));
    assert_eq!(parse_json(&body).unwrap().len(), MAX_CUES);
}

#[test]
fn a_long_aux_label_is_capped() {
    let room = Room::default();
    let state = room.apply(
        &Command::AuxSet {
            label: Some(long(LINE_TEXT_LIMIT + 60)),
            visible: Some(true),
        },
        T0,
    );
    assert_eq!(state.aux.label.chars().count(), LINE_TEXT_LIMIT);
}
