use simple_confidence_monitor::room::{Command, CueDraft, Room};
use simple_confidence_monitor::rundown_io::{parse_csv, parse_duration, to_csv};

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn draft(title: &str, minutes: u64) -> CueDraft {
    CueDraft {
        title: title.to_string(),
        speaker: String::new(),
        duration_ms: minutes * MIN,
        notes: String::new(),
    }
}

#[test]
fn parses_a_duration_in_minutes() {
    assert_eq!(parse_duration("15"), Some(15 * MIN));
    assert_eq!(parse_duration(" 5 "), Some(5 * MIN));
    assert_eq!(parse_duration("0"), Some(0));
}

#[test]
fn parses_a_duration_as_minutes_and_seconds() {
    assert_eq!(parse_duration("15:00"), Some(15 * MIN));
    assert_eq!(parse_duration("1:30"), Some(90_000));
}

#[test]
fn parses_a_duration_with_hours() {
    assert_eq!(parse_duration("1:02:03"), Some(3_723_000));
}

#[test]
fn refuses_text_that_is_not_a_duration() {
    for bad in ["", "  ", "abc", "5m", "1:2:3:4", "-5", "1:-2"] {
        assert_eq!(parse_duration(bad), None, "expected {bad} to be refused");
    }
}

#[test]
fn reads_a_csv_with_a_header() {
    let csv = "title,speaker,duration,notes\nWelcome,Kyle,5:00,say hello\nKeynote,Alice,30,\n";
    let cues = parse_csv(csv).unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].title, "Welcome");
    assert_eq!(cues[0].speaker, "Kyle");
    assert_eq!(cues[0].duration_ms, 5 * MIN);
    assert_eq!(cues[0].notes, "say hello");
    assert_eq!(cues[1].duration_ms, 30 * MIN);
}

#[test]
fn reads_a_csv_without_a_header() {
    let cues = parse_csv("Welcome,Kyle,5:00\n").unwrap();
    assert_eq!(cues.len(), 1);
    assert_eq!(cues[0].title, "Welcome");
}

#[test]
fn reads_columns_in_the_order_the_header_names() {
    let csv = "duration,title\n5:00,Welcome\n";
    let cues = parse_csv(csv).unwrap();
    assert_eq!(cues[0].title, "Welcome");
    assert_eq!(cues[0].duration_ms, 5 * MIN);
}

#[test]
fn reads_a_quoted_field_holding_a_comma() {
    let csv = "title,speaker,duration\n\"Panel: A, B and C\",Alice,20\n";
    let cues = parse_csv(csv).unwrap();
    assert_eq!(cues[0].title, "Panel: A, B and C");
}

#[test]
fn reads_a_doubled_quote_as_one_quote() {
    let cues = parse_csv("title,duration\n\"The \"\"big\"\" one\",10\n").unwrap();
    assert_eq!(cues[0].title, "The \"big\" one");
}

#[test]
fn skips_a_blank_line() {
    let cues = parse_csv("title,duration\n\nWelcome,5\n\n").unwrap();
    assert_eq!(cues.len(), 1);
}

#[test]
fn refuses_a_row_with_no_title() {
    let error = parse_csv("title,duration\n,5\n").unwrap_err();
    assert!(error.contains("line 2"), "got {error}");
}

#[test]
fn refuses_a_row_with_a_bad_duration() {
    let error = parse_csv("title,duration\nWelcome,soon\n").unwrap_err();
    assert!(error.contains("line 2"), "got {error}");
    assert!(error.contains("soon"), "got {error}");
}

#[test]
fn refuses_an_empty_document() {
    assert!(parse_csv("").is_err());
    assert!(parse_csv("title,speaker,duration\n").is_err());
}

#[test]
fn a_row_without_a_duration_takes_the_default() {
    let cues = parse_csv("title,speaker\nWelcome,Kyle\n").unwrap();
    assert_eq!(cues[0].duration_ms, 5 * MIN);
}

#[test]
fn writes_a_csv_that_reads_back_the_same() {
    let room = Room::default();
    room.apply(
        &Command::SetCues {
            cues: vec![
                CueDraft {
                    title: "Panel: A, B".into(),
                    speaker: "Alice \"AJ\" Brown".into(),
                    duration_ms: 20 * MIN,
                    notes: "two mics".into(),
                },
                draft("Keynote", 30),
            ],
        },
        T0,
    );
    let csv = to_csv(&room.snapshot().rundown.cues);
    assert!(
        csv.starts_with("title,speaker,duration,notes\n"),
        "got {csv}"
    );

    let back = parse_csv(&csv).unwrap();
    assert_eq!(back.len(), 2);
    assert_eq!(back[0].title, "Panel: A, B");
    assert_eq!(back[0].speaker, "Alice \"AJ\" Brown");
    assert_eq!(back[0].duration_ms, 20 * MIN);
    assert_eq!(back[0].notes, "two mics");
}

#[test]
fn set_cues_replaces_the_whole_list_with_fresh_ids() {
    let room = Room::default();
    room.apply(
        &Command::AddCue {
            title: Some("Old".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    let old_id = room.snapshot().rundown.cues[0].id;

    let state = room.apply(
        &Command::SetCues {
            cues: vec![draft("New", 10)],
        },
        T0,
    );
    assert_eq!(state.rundown.cues.len(), 1);
    assert_eq!(state.rundown.cues[0].title, "New");
    assert_ne!(state.rundown.cues[0].id, old_id);
}

#[test]
fn set_cues_clears_an_active_cue_that_is_gone() {
    let room = Room::default();
    room.apply(
        &Command::AddCue {
            title: Some("Old".into()),
            speaker: None,
            duration_ms: None,
            notes: None,
        },
        T0,
    );
    let id = room.snapshot().rundown.cues[0].id;
    room.apply(&Command::LoadCue { id }, T0);

    let state = room.apply(
        &Command::SetCues {
            cues: vec![draft("New", 10)],
        },
        T0,
    );
    assert_eq!(state.rundown.active, None);
}

#[test]
fn set_cues_with_an_identical_list_still_replaces_the_ids() {
    let room = Room::default();
    room.apply(
        &Command::SetCues {
            cues: vec![draft("One", 5)],
        },
        T0,
    );
    let first = room.snapshot().rundown.cues[0].id;
    room.apply(
        &Command::SetCues {
            cues: vec![draft("One", 5)],
        },
        T0,
    );
    assert_ne!(room.snapshot().rundown.cues[0].id, first);
}

#[test]
fn parses_the_set_cues_command() {
    let command: Command = serde_json::from_str(
        r#"{"cmd":"set_cues","cues":[{"title":"Welcome","duration_ms":300000}]}"#,
    )
    .unwrap();
    assert_eq!(
        command,
        Command::SetCues {
            cues: vec![draft("Welcome", 5)]
        }
    );
}

#[test]
fn a_field_holding_a_newline_survives_a_round_trip() {
    let room = Room::default();
    room.apply(
        &Command::SetCues {
            cues: vec![CueDraft {
                title: "Keynote".into(),
                speaker: "Alice".into(),
                duration_ms: 30 * MIN,
                notes: "line one\nline two".into(),
            }],
        },
        T0,
    );
    let csv = to_csv(&room.snapshot().rundown.cues);

    let back = parse_csv(&csv).unwrap();
    assert_eq!(
        back.len(),
        1,
        "a quoted newline must not split the row: {csv:?}"
    );
    assert_eq!(back[0].title, "Keynote");
    assert_eq!(back[0].notes, "line one\nline two");
}

#[test]
fn a_quoted_newline_in_the_middle_of_a_document_reads_as_one_row() {
    let csv = "title,speaker,duration,notes\n\"Panel\",Alice,20,\"first\nsecond\"\nBreak,,10,\n";
    let cues = parse_csv(csv).unwrap();
    assert_eq!(cues.len(), 2);
    assert_eq!(cues[0].notes, "first\nsecond");
    assert_eq!(cues[1].title, "Break");
}

#[test]
fn a_line_number_in_an_error_points_at_the_row_start() {
    let csv = "title,duration\n\"Panel\nwith a newline\",20\nBroken,soon\n";
    let error = parse_csv(csv).unwrap_err();
    assert!(error.contains("line 4"), "got {error}");
}

#[test]
fn refuses_a_document_with_an_unclosed_quote() {
    // One stray quote must not swallow every row after it.
    let csv = "title,duration\nPanel\",20\nBreak,10\nClosing,15\n";
    let error = parse_csv(csv).unwrap_err();
    assert!(error.contains("quote"), "got {error}");
    assert!(error.contains("line 2"), "got {error}");
}

#[test]
fn refuses_a_quote_left_open_at_the_end() {
    let error = parse_csv("title,duration\n\"Panel,20\n").unwrap_err();
    assert!(error.contains("quote"), "got {error}");
}

#[test]
fn a_closed_quote_at_the_end_of_a_document_is_fine() {
    let cues = parse_csv("title,notes\nPanel,\"two mics\"\n").unwrap();
    assert_eq!(cues[0].notes, "two mics");
}
