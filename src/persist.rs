use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::sync::Notify;

use crate::clock::now_ms;
use crate::hub::Hub;
use crate::room::{RoomName, RoomState};
use crate::timer::Run;

/// A room on disk. saved_at_ms lets a reload fold a running timer into elapsed time.
#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub saved_at_ms: u64,
    pub state: RoomState,
}

/// Room snapshots in a directory, one JSON file per room.
#[derive(Debug)]
pub struct Store {
    dir: PathBuf,
}

impl Store {
    pub fn new(dir: impl Into<PathBuf>) -> std::io::Result<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir)?;
        let store = Self { dir };
        store.probe()?;
        Ok(store)
    }

    /// A bind mount can arrive read-only for this user, and then every save
    /// fails long after the operator stopped reading the log.
    fn probe(&self) -> std::io::Result<()> {
        let path = self.dir.join(".writable");
        std::fs::write(&path, b"")?;
        std::fs::remove_file(&path)
    }

    pub fn path_for(&self, name: &RoomName) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    /// Writes through a temporary file, so a crash cannot leave half a snapshot.
    pub fn save(
        &self,
        name: &RoomName,
        state: &RoomState,
        saved_at_ms: u64,
    ) -> std::io::Result<()> {
        let snapshot = Snapshot {
            saved_at_ms,
            state: state.clone(),
        };
        let body = serde_json::to_vec_pretty(&snapshot)
            .map_err(|err| std::io::Error::new(std::io::ErrorKind::InvalidData, err))?;
        let target = self.path_for(name);
        let temporary = target.with_extension("json.tmp");
        std::fs::write(&temporary, body)?;
        std::fs::rename(&temporary, &target)
    }

    /// Every readable snapshot in the directory. Unreadable files are skipped.
    pub fn load_all(&self) -> Vec<(RoomName, RoomState)> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut rooms = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "json") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let Ok(name) = RoomName::parse(stem) else {
                continue;
            };
            let Ok(body) = std::fs::read(&path) else {
                continue;
            };
            let Ok(snapshot) = serde_json::from_slice::<Snapshot>(&body) else {
                continue;
            };
            rooms.push((name, restored(snapshot)));
        }
        rooms.sort_by(|left, right| left.0.cmp(&right.0));
        rooms
    }
}

/// A restart means the show already stopped, so a running timer comes back
/// paused. Both timers, or the second one would count the downtime.
fn restored(snapshot: Snapshot) -> RoomState {
    let mut state = snapshot.state;
    pause_at_save(&mut state.timer, snapshot.saved_at_ms);
    pause_at_save(&mut state.aux.timer, snapshot.saved_at_ms);
    state
}

fn pause_at_save(timer: &mut crate::timer::Timer, saved_at_ms: u64) {
    if let Run::Running { since_ms } = timer.run {
        timer.elapsed_ms += saved_at_ms.saturating_sub(since_ms);
        timer.run = Run::Paused;
    }
}

/// Debounced writer. A room marks itself dirty, and the flusher writes it once.
#[derive(Debug)]
pub struct Snapshots {
    store: Store,
    dirty: Mutex<HashSet<RoomName>>,
    wake: Notify,
}

impl Snapshots {
    pub fn new(store: Store) -> Self {
        Self {
            store,
            dirty: Mutex::new(HashSet::new()),
            wake: Notify::new(),
        }
    }

    pub fn mark(&self, name: &RoomName) {
        self.dirty.lock().expect("dirty lock").insert(name.clone());
        self.wake.notify_one();
    }

    pub fn pending(&self) -> usize {
        self.dirty.lock().expect("dirty lock").len()
    }

    /// Writes every dirty room now.
    pub fn flush(&self, hub: &Hub) {
        let names: Vec<RoomName> = self.dirty.lock().expect("dirty lock").drain().collect();
        let saved_at_ms = now_ms();
        for name in names {
            let Some(room) = hub.get(&name) else { continue };
            if let Err(err) = self.store.save(&name, &room.snapshot(), saved_at_ms) {
                tracing::warn!("could not save room {name}: {err}");
            }
        }
    }

    /// Drops a room from the pending set and deletes its snapshot.
    pub fn forget(&self, name: &RoomName) {
        self.dirty.lock().expect("dirty lock").remove(name);
        let path = self.store.path_for(name);
        if let Err(err) = std::fs::remove_file(&path)
            && err.kind() != std::io::ErrorKind::NotFound
        {
            tracing::warn!("could not delete snapshot {path:?}: {err}");
        }
    }

    /// Waits for a change, settles for the debounce window, then writes.
    pub async fn run(&self, hub: &Hub, debounce: Duration) {
        loop {
            self.wake.notified().await;
            tokio::time::sleep(debounce).await;
            self.flush(hub);
        }
    }
}
