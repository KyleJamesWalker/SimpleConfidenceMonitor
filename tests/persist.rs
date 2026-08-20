use std::time::Duration;

use simple_confidence_monitor::clock::now_ms;
use simple_confidence_monitor::hub::Hub;
use simple_confidence_monitor::persist::{Snapshots, Store};
use simple_confidence_monitor::room::{Command, RoomName, RoomState};
use simple_confidence_monitor::timer::Run;

const T0: u64 = 1_700_000_000_000;

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("scm-test-{tag}-{}", now_ms()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn name(raw: &str) -> RoomName {
    RoomName::parse(raw).unwrap()
}

#[test]
fn new_creates_the_directory() {
    let dir = temp_dir("create").join("nested");
    assert!(!dir.exists());
    Store::new(&dir).unwrap();
    assert!(dir.is_dir());
}

#[test]
fn save_writes_one_file_per_room() {
    let dir = temp_dir("save");
    let store = Store::new(&dir).unwrap();
    store
        .save(&name("keynote"), &RoomState::default(), T0)
        .unwrap();
    assert!(dir.join("keynote.json").is_file());
    assert_eq!(std::fs::read_dir(&dir).unwrap().count(), 1);
}

#[test]
fn a_saved_room_loads_back() {
    let dir = temp_dir("round");
    let store = Store::new(&dir).unwrap();
    let mut state = RoomState {
        rev: 7,
        ..RoomState::default()
    };
    state.timer.duration_ms = 300_000;
    state.display.title = "Keynote".into();
    state.message.text = "Wrap up".into();
    store.save(&name("keynote"), &state, T0).unwrap();

    let loaded = store.load_all();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0, name("keynote"));
    assert_eq!(loaded[0].1.rev, 7);
    assert_eq!(loaded[0].1.timer.duration_ms, 300_000);
    assert_eq!(loaded[0].1.display.title, "Keynote");
    assert_eq!(loaded[0].1.message.text, "Wrap up");
}

#[test]
fn a_running_room_reloads_paused_with_its_elapsed_time() {
    let dir = temp_dir("running");
    let store = Store::new(&dir).unwrap();
    let mut state = RoomState::default();
    state.timer.run = Run::Running { since_ms: T0 };
    store.save(&name("keynote"), &state, T0 + 90_000).unwrap();

    let loaded = store.load_all();
    assert_eq!(loaded[0].1.timer.run, Run::Paused);
    assert_eq!(loaded[0].1.timer.elapsed_ms, 90_000);
}

#[test]
fn a_paused_room_keeps_its_elapsed_time() {
    let dir = temp_dir("paused");
    let store = Store::new(&dir).unwrap();
    let mut state = RoomState::default();
    state.timer.run = Run::Paused;
    state.timer.elapsed_ms = 45_000;
    store.save(&name("keynote"), &state, T0 + 90_000).unwrap();
    assert_eq!(store.load_all()[0].1.timer.elapsed_ms, 45_000);
}

#[test]
fn load_all_skips_files_it_cannot_use() {
    let dir = temp_dir("junk");
    let store = Store::new(&dir).unwrap();
    store
        .save(&name("good"), &RoomState::default(), T0)
        .unwrap();
    std::fs::write(dir.join("broken.json"), "{not json").unwrap();
    std::fs::write(dir.join("notes.txt"), "hello").unwrap();
    std::fs::write(dir.join("Bad Name.json"), "{}").unwrap();

    let loaded = store.load_all();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].0, name("good"));
}

#[test]
fn load_all_on_an_empty_directory_returns_nothing() {
    let store = Store::new(temp_dir("empty")).unwrap();
    assert!(store.load_all().is_empty());
}

#[test]
fn save_leaves_no_temporary_file_behind() {
    let dir = temp_dir("atomic");
    let store = Store::new(&dir).unwrap();
    store
        .save(&name("keynote"), &RoomState::default(), T0)
        .unwrap();
    let names: Vec<String> = std::fs::read_dir(&dir)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().to_string())
        .collect();
    assert_eq!(names, vec!["keynote.json".to_string()]);
}

#[test]
fn marking_a_room_makes_it_pending_once() {
    let snapshots = Snapshots::new(Store::new(temp_dir("mark")).unwrap());
    snapshots.mark(&name("keynote"));
    snapshots.mark(&name("keynote"));
    assert_eq!(snapshots.pending(), 1);
    snapshots.mark(&name("breakout"));
    assert_eq!(snapshots.pending(), 2);
}

#[test]
fn flush_writes_the_pending_rooms_and_clears_them() {
    let dir = temp_dir("flush");
    let snapshots = Snapshots::new(Store::new(&dir).unwrap());
    let hub = Hub::new();
    hub.get_or_create(&name("keynote"))
        .apply(&Command::SetDuration { ms: 60_000 }, T0);
    snapshots.mark(&name("keynote"));
    snapshots.flush(&hub);

    assert_eq!(snapshots.pending(), 0);
    assert!(dir.join("keynote.json").is_file());
}

#[test]
fn flush_ignores_a_room_the_hub_no_longer_holds() {
    let dir = temp_dir("gone");
    let snapshots = Snapshots::new(Store::new(&dir).unwrap());
    snapshots.mark(&name("ghost"));
    snapshots.flush(&Hub::new());
    assert_eq!(snapshots.pending(), 0);
    assert!(!dir.join("ghost.json").exists());
}

#[tokio::test]
async fn a_command_reaches_disk_through_the_flusher() {
    let dir = temp_dir("wired");
    let snapshots = std::sync::Arc::new(Snapshots::new(Store::new(&dir).unwrap()));
    let hub = Hub::with_snapshots(snapshots.clone());
    let flusher = {
        let hub = hub.clone();
        let snapshots = snapshots.clone();
        tokio::spawn(async move { snapshots.run(&hub, Duration::from_millis(10)).await })
    };

    hub.get_or_create(&name("keynote"))
        .apply(&Command::SetDuration { ms: 120_000 }, now_ms());
    tokio::time::sleep(Duration::from_millis(120)).await;
    flusher.abort();

    let loaded = Store::new(&dir).unwrap().load_all();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].1.timer.duration_ms, 120_000);
}

#[test]
fn a_hub_restores_the_rooms_it_finds() {
    let dir = temp_dir("restore");
    let store = Store::new(&dir).unwrap();
    let mut state = RoomState::default();
    state.timer.duration_ms = 42_000;
    store.save(&name("keynote"), &state, T0).unwrap();

    let hub = Hub::new();
    hub.restore(store.load_all());
    assert_eq!(hub.room_count(), 1);
    let room = hub.get_or_create(&name("keynote"));
    assert_eq!(room.snapshot().timer.duration_ms, 42_000);
}

#[test]
fn a_snapshot_written_before_a_field_existed_still_loads() {
    let dir = temp_dir("older");
    let store = Store::new(&dir).unwrap();
    // A snapshot from before scheduled starts and presets existed.
    let body = r#"{
      "saved_at_ms": 1700000000000,
      "state": {
        "rev": 3,
        "timer": {
          "mode": "countdown",
          "duration_ms": 600000,
          "run": {"state": "stopped"},
          "elapsed_ms": 0,
          "warn_ms": 180000,
          "danger_ms": 60000,
          "on_expire": "count_negative"
        },
        "message": {"text": "", "tone": "neutral", "visible": false},
        "display": {
          "title": "Keynote", "next_up": "", "show_clock": true, "clock_24h": true,
          "show_progress": true, "blackout": false, "mirror": false, "scale": 100,
          "flash_at": 0
        },
        "rundown": {"cues": [], "active": null, "auto_advance": false, "next_id": 0}
      }
    }"#;
    std::fs::write(dir.join("keynote.json"), body).unwrap();

    let loaded = store.load_all();
    assert_eq!(loaded.len(), 1, "an older snapshot should still load");
    assert_eq!(loaded[0].1.display.title, "Keynote");
    assert_eq!(loaded[0].1.timer.start_at_ms, None);
    assert!(!loaded[0].1.display.chime);
}

#[test]
fn forgetting_a_room_deletes_its_snapshot() {
    let dir = temp_dir("forget");
    let snapshots = Snapshots::new(Store::new(&dir).unwrap());
    let hub = Hub::new();
    hub.get_or_create(&name("keynote"))
        .apply(&Command::SetDuration { ms: 60_000 }, T0);
    snapshots.mark(&name("keynote"));
    snapshots.flush(&hub);
    assert!(dir.join("keynote.json").is_file());

    snapshots.forget(&name("keynote"));
    assert!(!dir.join("keynote.json").exists());
}

#[test]
fn forgetting_a_room_drops_it_from_the_pending_set() {
    let dir = temp_dir("forget-pending");
    let snapshots = Snapshots::new(Store::new(&dir).unwrap());
    snapshots.mark(&name("keynote"));
    snapshots.forget(&name("keynote"));
    assert_eq!(snapshots.pending(), 0);
    snapshots.flush(&Hub::new());
    assert!(!dir.join("keynote.json").exists());
}

#[test]
fn a_running_aux_timer_also_reloads_paused() {
    let dir = temp_dir("aux-running");
    let store = Store::new(&dir).unwrap();
    let mut state = RoomState::default();
    state.aux.timer.run = Run::Running { since_ms: T0 };
    state.aux.visible = true;
    store.save(&name("keynote"), &state, T0 + 90_000).unwrap();

    let loaded = store.load_all();
    assert_eq!(
        loaded[0].1.aux.timer.run,
        Run::Paused,
        "a restart stops the show, and that includes the second timer"
    );
    assert_eq!(loaded[0].1.aux.timer.elapsed_ms, 90_000);
}
