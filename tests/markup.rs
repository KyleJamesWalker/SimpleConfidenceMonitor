//! The pages ship embedded, so their markup is worth asserting on. A toggle
//! that carries its state only as a CSS class tells a screen reader nothing.

const CONSOLE: &str = include_str!("../web/console.html");
const VIEWER: &str = include_str!("../web/viewer.html");
const UNLOCK: &str = include_str!("../web/unlock.html");
const AGENDA: &str = include_str!("../web/agenda.html");
const PICKER: &str = include_str!("../web/picker.html");

fn ids(page: &str) -> Vec<&str> {
    page.match_indices("id=\"")
        .filter_map(|(at, _)| {
            let rest = &page[at + 4..];
            rest.find('"').map(|end| &rest[..end])
        })
        .collect()
}

#[test]
fn every_toggle_reports_whether_it_is_on() {
    let toggles: Vec<&str> = CONSOLE
        .lines()
        .filter(|line| line.contains("data-toggle"))
        .collect();
    assert!(toggles.len() >= 10, "the console has ten screen toggles");
    for line in toggles {
        assert!(
            line.contains("aria-pressed"),
            "a toggle carries its state only as a class: {}",
            line.trim()
        );
    }
}

#[test]
fn every_tone_button_reports_whether_it_is_on() {
    for line in CONSOLE
        .lines()
        .filter(|line| line.contains("class=\"tone\""))
    {
        assert!(
            line.contains("aria-pressed"),
            "a tone button carries no state: {}",
            line.trim()
        );
    }
}

#[test]
fn the_toast_and_the_connection_badge_are_live_regions() {
    for line in CONSOLE
        .lines()
        .filter(|line| line.contains("id=\"toast\"") || line.contains("id=\"status\""))
    {
        assert!(line.contains("aria-live"), "{}", line.trim());
    }
}

#[test]
fn the_viewer_badge_is_a_live_region_and_its_icon_button_has_a_label() {
    let badge = VIEWER
        .lines()
        .find(|line| line.contains("id=\"status\""))
        .expect("the viewer has a reconnecting badge");
    assert!(badge.contains("aria-live"), "{}", badge.trim());
    let fullscreen = VIEWER
        .lines()
        .find(|line| line.contains("id=\"fullscreen\""))
        .expect("the viewer has a fullscreen button");
    assert!(
        fullscreen.contains("aria-label"),
        "an icon is not a label: {}",
        fullscreen.trim()
    );
}

// A speaker reads a note off the stage display, so it belongs under the timer
// rather than in the footer beside the crew lines.
#[test]
fn the_speaker_note_sits_under_the_timer() {
    let middle = VIEWER
        .split("<main class=\"middle\">")
        .nth(1)
        .and_then(|rest| rest.split("</main>").next())
        .expect("the viewer has a middle block");
    assert!(
        middle.contains("id=\"notes\""),
        "the note must sit with the timer, not in the footer"
    );
    let timer_first = middle.find("id=\"timer\"") < middle.find("id=\"notes\"");
    assert!(timer_first, "the timer stays above the note");
}

#[test]
fn the_console_links_back_to_the_picker() {
    assert!(
        CONSOLE.contains("id=\"pickerLink\""),
        "switching rooms must not mean editing the URL by hand"
    );
}

#[test]
fn a_picker_link_is_never_labelled_as_the_viewer_link() {
    for (page, body) in [("console.html", CONSOLE), ("unlock.html", UNLOCK)] {
        for line in body.lines().filter(|line| line.contains("room picker")) {
            assert!(
                !line.contains("id=\"viewerLink\""),
                "{page} labels a picker link as the viewer link: {}",
                line.trim()
            );
        }
    }
}

#[test]
fn no_page_repeats_an_id() {
    for (page, body) in [
        ("console.html", CONSOLE),
        ("viewer.html", VIEWER),
        ("unlock.html", UNLOCK),
        ("agenda.html", AGENDA),
        ("picker.html", PICKER),
    ] {
        let mut seen = ids(body);
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        assert_eq!(count, seen.len(), "{page} repeats an id");
    }
}
