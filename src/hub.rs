use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::persist::Snapshots;
use crate::room::{Room, RoomName, RoomState};

/// Registry of live rooms. Rooms are created on first reference.
#[derive(Default)]
pub struct Hub {
    rooms: RwLock<HashMap<RoomName, Arc<Room>>>,
    snapshots: Option<Arc<Snapshots>>,
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
    }

    pub fn with_snapshots(snapshots: Arc<Snapshots>) -> Arc<Self> {
        Arc::new(Self {
            rooms: RwLock::new(HashMap::new()),
            snapshots: Some(snapshots),
        })
    }

    /// Returns the room, creating it when absent.
    pub fn get_or_create(&self, name: &RoomName) -> Arc<Room> {
        if let Some(room) = self.rooms.read().expect("hub lock").get(name) {
            return room.clone();
        }
        self.rooms
            .write()
            .expect("hub lock")
            .entry(name.clone())
            .or_insert_with(|| Arc::new(Room::new(name.clone(), self.snapshots.clone())))
            .clone()
    }

    pub fn get(&self, name: &RoomName) -> Option<Arc<Room>> {
        self.rooms.read().expect("hub lock").get(name).cloned()
    }

    /// Puts loaded rooms back in place at startup.
    pub fn restore(&self, rooms: Vec<(RoomName, RoomState)>) {
        let mut live = self.rooms.write().expect("hub lock");
        for (name, state) in rooms {
            let room = Room::restored(name.clone(), state, self.snapshots.clone());
            live.insert(name, Arc::new(room));
        }
    }

    pub fn room_count(&self) -> usize {
        self.rooms.read().expect("hub lock").len()
    }

    pub fn room_names(&self) -> Vec<RoomName> {
        let mut names: Vec<RoomName> = self
            .rooms
            .read()
            .expect("hub lock")
            .keys()
            .cloned()
            .collect();
        names.sort();
        names
    }
}
