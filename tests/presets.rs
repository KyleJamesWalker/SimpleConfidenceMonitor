use simple_confidence_monitor::room::{
    Command, MAX_PRESETS, PRESET_TEXT_LIMIT, Preset, Room, Tone,
};

const T0: u64 = 1_700_000_000_000;

fn parse(json: &str) -> Command {
    serde_json::from_str(json).expect("command should parse")
}

fn preset(text: &str, tone: Tone) -> Preset {
    Preset {
        text: text.to_string(),
        tone,
    }
}

#[test]
fn a_new_room_carries_the_default_presets() {
    let presets = Room::default().snapshot().presets;
    assert!(!presets.is_empty(), "an operator needs something to press");
    assert!(presets.iter().all(|preset| !preset.text.is_empty()));
    assert!(presets.iter().any(|preset| preset.tone == Tone::Alert));
}

#[test]
fn sending_a_preset_shows_it_with_its_tone() {
    let room = Room::default();
    room.apply(
        &Command::SetPresets {
            presets: vec![preset("Wrap up", Tone::Warn)],
        },
        T0,
    );
    let state = room.apply(&Command::SendPreset { index: 0 }, T0);
    assert_eq!(state.message.text, "Wrap up");
    assert_eq!(state.message.tone, Tone::Warn);
    assert!(state.message.visible);
}

#[test]
fn sending_a_preset_that_is_not_there_changes_nothing() {
    let room = Room::default();
    let before = room.snapshot();
    let state = room.apply(&Command::SendPreset { index: 99 }, T0);
    assert_eq!(state.rev, before.rev);
    assert_eq!(state.message.text, "");
}

#[test]
fn resending_the_same_preset_does_not_bump_rev() {
    let room = Room::default();
    let first = room.apply(&Command::SendPreset { index: 0 }, T0);
    assert_eq!(
        room.apply(&Command::SendPreset { index: 0 }, T0).rev,
        first.rev
    );
}

#[test]
fn setting_presets_replaces_the_list() {
    let room = Room::default();
    let state = room.apply(
        &Command::SetPresets {
            presets: vec![preset("One", Tone::Neutral), preset("Two", Tone::Alert)],
        },
        T0,
    );
    assert_eq!(state.presets.len(), 2);
    assert_eq!(state.presets[1].text, "Two");
    assert_eq!(state.presets[1].tone, Tone::Alert);
}

#[test]
fn setting_presets_drops_the_ones_past_the_cap() {
    let room = Room::default();
    let many = (0..MAX_PRESETS + 5)
        .map(|index| preset(&format!("Preset {index}"), Tone::Neutral))
        .collect();
    let state = room.apply(&Command::SetPresets { presets: many }, T0);
    assert_eq!(state.presets.len(), MAX_PRESETS);
}

#[test]
fn setting_presets_trims_text_to_the_limit() {
    let room = Room::default();
    let long = "x".repeat(PRESET_TEXT_LIMIT + 50);
    let state = room.apply(
        &Command::SetPresets {
            presets: vec![preset(&long, Tone::Neutral)],
        },
        T0,
    );
    assert_eq!(state.presets[0].text.chars().count(), PRESET_TEXT_LIMIT);
}

#[test]
fn setting_presets_drops_an_empty_one() {
    let room = Room::default();
    let state = room.apply(
        &Command::SetPresets {
            presets: vec![preset("  ", Tone::Neutral), preset("Wrap up", Tone::Warn)],
        },
        T0,
    );
    assert_eq!(state.presets.len(), 1);
    assert_eq!(state.presets[0].text, "Wrap up");
}

#[test]
fn parses_the_preset_commands() {
    assert_eq!(
        parse(r#"{"cmd":"send_preset","index":2}"#),
        Command::SendPreset { index: 2 }
    );
    assert_eq!(
        parse(r#"{"cmd":"set_presets","presets":[{"text":"Wrap up","tone":"warn"}]}"#),
        Command::SetPresets {
            presets: vec![preset("Wrap up", Tone::Warn)]
        }
    );
}

#[test]
fn a_preset_without_a_tone_reads_as_neutral() {
    assert_eq!(
        parse(r#"{"cmd":"set_presets","presets":[{"text":"Wrap up"}]}"#),
        Command::SetPresets {
            presets: vec![preset("Wrap up", Tone::Neutral)]
        }
    );
}
