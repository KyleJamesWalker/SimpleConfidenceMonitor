use std::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::clock::now_ms;
use crate::timer::{Mode, OnExpire, Timer};
use crate::wire::ServerMsg;

/// A validated room name, safe in a URL path and as a snapshot filename.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RoomName(String);

pub const RESERVED_NAMES: [&str; 3] = ["api", "assets", "healthz"];
pub const MAX_NAME_LEN: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NameError {
    Empty,
    TooLong,
    Reserved,
    BadCharacter,
}

impl std::fmt::Display for NameError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Self::Empty => "room name is empty",
            Self::TooLong => "room name is longer than 64 characters",
            Self::Reserved => "room name is reserved",
            Self::BadCharacter => "room name allows only a-z, 0-9, dash and underscore",
        };
        f.write_str(text)
    }
}

impl RoomName {
    pub fn parse(raw: &str) -> Result<Self, NameError> {
        if raw.is_empty() {
            return Err(NameError::Empty);
        }
        if raw.chars().count() > MAX_NAME_LEN {
            return Err(NameError::TooLong);
        }
        let name = raw.to_ascii_lowercase();
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        {
            return Err(NameError::BadCharacter);
        }
        if RESERVED_NAMES.contains(&name.as_str()) {
            return Err(NameError::Reserved);
        }
        Ok(Self(name))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for RoomName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Everything a viewer needs to render the show.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoomState {
    /// Increments on every command that changes state.
    pub rev: u64,
    pub timer: Timer,
}

/// One operator action. The same envelope arrives over the socket and over HTTP.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Start,
    Pause,
    Reset,
    SetDuration { ms: u64 },
    Adjust { ms: i64 },
    SetMode { mode: Mode },
    SetThresholds { warn_ms: u64, danger_ms: u64 },
    SetOnExpire { on_expire: OnExpire },
}

impl RoomState {
    /// Applies one command. Returns true when the state changed.
    pub fn apply(&mut self, cmd: &Command, now_ms: u64) -> bool {
        let timer = &mut self.timer;
        match cmd {
            Command::Start => timer.start(now_ms),
            Command::Pause => timer.pause(now_ms),
            Command::Reset => timer.reset(),
            Command::SetDuration { ms } => timer.set_duration(*ms),
            Command::Adjust { ms } => timer.adjust(*ms),
            Command::SetMode { mode } => timer.set_mode(*mode),
            Command::SetThresholds { warn_ms, danger_ms } => {
                let changed = timer.warn_ms != *warn_ms || timer.danger_ms != *danger_ms;
                timer.warn_ms = *warn_ms;
                timer.danger_ms = *danger_ms;
                changed
            }
            Command::SetOnExpire { on_expire } => {
                let changed = timer.on_expire != *on_expire;
                timer.on_expire = *on_expire;
                changed
            }
        }
    }
}

/// How many state frames a slow client may fall behind before it resynchronizes.
const FRAME_BACKLOG: usize = 32;

/// A live room. Shared across every connected client.
#[derive(Debug)]
pub struct Room {
    state: Mutex<RoomState>,
    frames: broadcast::Sender<String>,
    viewers: AtomicUsize,
    editors: AtomicUsize,
}

impl Default for Room {
    fn default() -> Self {
        Self {
            state: Mutex::new(RoomState::default()),
            frames: broadcast::channel(FRAME_BACKLOG).0,
            viewers: AtomicUsize::new(0),
            editors: AtomicUsize::new(0),
        }
    }
}

impl Room {
    pub fn snapshot(&self) -> RoomState {
        self.state.lock().expect("room lock").clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<String> {
        self.frames.subscribe()
    }

    pub fn viewers(&self) -> usize {
        self.viewers.load(Ordering::Relaxed)
    }

    pub fn editors(&self) -> usize {
        self.editors.load(Ordering::Relaxed)
    }

    /// The current state as a wire frame, stamped with the server clock.
    pub fn frame(&self) -> String {
        let state = self.state.lock().expect("room lock");
        let msg = ServerMsg::State {
            server_time_ms: now_ms(),
            viewers: self.viewers(),
            editors: self.editors(),
            state: &state,
        };
        serde_json::to_string(&msg).expect("state frame serializes")
    }

    /// Applies a command, tells every client, and returns the new state.
    pub fn apply(&self, cmd: &Command, now_ms: u64) -> RoomState {
        let next = {
            let mut state = self.state.lock().expect("room lock");
            if state.apply(cmd, now_ms) {
                state.rev += 1;
            }
            state.clone()
        };
        self.publish();
        next
    }

    /// Sends the current state to every subscriber. A room with no client is a no-op.
    pub fn publish(&self) {
        let _ = self.frames.send(self.frame());
    }

    pub fn client_joined(&self, editor: bool) {
        self.counter(editor).fetch_add(1, Ordering::Relaxed);
        self.publish();
    }

    pub fn client_left(&self, editor: bool) {
        self.counter(editor).fetch_sub(1, Ordering::Relaxed);
        self.publish();
    }

    fn counter(&self, editor: bool) -> &AtomicUsize {
        if editor { &self.editors } else { &self.viewers }
    }
}
