use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::room::{Room, RoomName};

/// Registry of live rooms. Rooms are created on first reference.
#[derive(Default)]
pub struct Hub {
    rooms: RwLock<HashMap<RoomName, Arc<Room>>>,
}

impl Hub {
    pub fn new() -> Arc<Self> {
        Arc::new(Self::default())
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
            .or_insert_with(|| Arc::new(Room::default()))
            .clone()
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
