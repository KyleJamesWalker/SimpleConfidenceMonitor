use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::clock::now_ms;
use crate::persist::Snapshots;
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

/// Emphasis for an operator note. The viewer colors the overlay by tone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Tone {
    #[default]
    Neutral,
    Warn,
    Alert,
}

/// A note to the speaker.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub text: String,
    pub tone: Tone,
    pub visible: bool,
}

/// The viewer settings that are neither the timer nor the message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Display {
    pub title: String,
    pub next_up: String,
    pub show_clock: bool,
    pub clock_24h: bool,
    pub show_progress: bool,
    pub blackout: bool,
    /// Horizontal flip, for a screen behind teleprompter glass.
    pub mirror: bool,
    pub scale: u8,
    /// A viewer flashes when this value changes.
    pub flash_at: u64,
}

pub const MIN_SCALE: u8 = 50;
pub const MAX_SCALE: u8 = 200;

impl Default for Display {
    fn default() -> Self {
        Self {
            title: String::new(),
            next_up: String::new(),
            show_clock: true,
            clock_24h: true,
            show_progress: true,
            blackout: false,
            mirror: false,
            scale: 100,
            flash_at: 0,
        }
    }
}

/// Everything a viewer needs to render the show.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct RoomState {
    /// Increments on every command that changes state.
    pub rev: u64,
    pub timer: Timer,
    pub message: Message,
    pub display: Display,
}

/// One operator action. The same envelope arrives over the socket and over HTTP.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Start,
    Pause,
    Reset,
    SetDuration {
        ms: u64,
    },
    Adjust {
        ms: i64,
    },
    SetMode {
        mode: Mode,
    },
    SetThresholds {
        warn_ms: u64,
        danger_ms: u64,
    },
    SetOnExpire {
        on_expire: OnExpire,
    },
    /// Every field is optional, so the console can change one of them.
    Message {
        text: Option<String>,
        tone: Option<Tone>,
        visible: Option<bool>,
    },
    Flash,
    Blackout {
        on: bool,
    },
    Display {
        title: Option<String>,
        next_up: Option<String>,
        show_clock: Option<bool>,
        clock_24h: Option<bool>,
        show_progress: Option<bool>,
        mirror: Option<bool>,
        scale: Option<u8>,
    },
}

impl RoomState {
    /// Applies one command. Returns true when the state changed.
    pub fn apply(&mut self, cmd: &Command, now_ms: u64) -> bool {
        match cmd {
            Command::Start => self.timer.start(now_ms),
            Command::Pause => self.timer.pause(now_ms),
            Command::Reset => self.timer.reset(),
            Command::SetDuration { ms } => self.timer.set_duration(*ms),
            Command::Adjust { ms } => self.timer.adjust(*ms),
            Command::SetMode { mode } => self.timer.set_mode(*mode),
            Command::SetThresholds { warn_ms, danger_ms } => {
                let changed = self.timer.warn_ms != *warn_ms || self.timer.danger_ms != *danger_ms;
                self.timer.warn_ms = *warn_ms;
                self.timer.danger_ms = *danger_ms;
                changed
            }
            Command::SetOnExpire { on_expire } => {
                let changed = self.timer.on_expire != *on_expire;
                self.timer.on_expire = *on_expire;
                changed
            }
            Command::Message {
                text,
                tone,
                visible,
            } => {
                let before = self.message.clone();
                if let Some(text) = text {
                    self.message.text = text.clone();
                }
                if let Some(tone) = tone {
                    self.message.tone = *tone;
                }
                if let Some(visible) = visible {
                    self.message.visible = *visible;
                }
                before != self.message
            }
            Command::Flash => {
                self.display.flash_at = now_ms;
                true
            }
            Command::Blackout { on } => {
                let changed = self.display.blackout != *on;
                self.display.blackout = *on;
                changed
            }
            Command::Display {
                title,
                next_up,
                show_clock,
                clock_24h,
                show_progress,
                mirror,
                scale,
            } => {
                let before = self.display.clone();
                if let Some(title) = title {
                    self.display.title = title.clone();
                }
                if let Some(next_up) = next_up {
                    self.display.next_up = next_up.clone();
                }
                if let Some(value) = show_clock {
                    self.display.show_clock = *value;
                }
                if let Some(value) = clock_24h {
                    self.display.clock_24h = *value;
                }
                if let Some(value) = show_progress {
                    self.display.show_progress = *value;
                }
                if let Some(value) = mirror {
                    self.display.mirror = *value;
                }
                if let Some(scale) = scale {
                    self.display.scale = (*scale).clamp(MIN_SCALE, MAX_SCALE);
                }
                before != self.display
            }
        }
    }
}

/// How many state frames a slow client may fall behind before it resynchronizes.
const FRAME_BACKLOG: usize = 32;

/// A live room. Shared across every connected client.
#[derive(Debug)]
pub struct Room {
    name: RoomName,
    state: Mutex<RoomState>,
    frames: broadcast::Sender<String>,
    viewers: AtomicUsize,
    editors: AtomicUsize,
    snapshots: Option<Arc<Snapshots>>,
}

impl Default for Room {
    fn default() -> Self {
        Self::new(
            RoomName::parse("room").expect("a literal name parses"),
            None,
        )
    }
}

impl Room {
    pub fn new(name: RoomName, snapshots: Option<Arc<Snapshots>>) -> Self {
        Self::restored(name, RoomState::default(), snapshots)
    }

    pub fn restored(name: RoomName, state: RoomState, snapshots: Option<Arc<Snapshots>>) -> Self {
        Self {
            name,
            state: Mutex::new(state),
            frames: broadcast::channel(FRAME_BACKLOG).0,
            viewers: AtomicUsize::new(0),
            editors: AtomicUsize::new(0),
            snapshots,
        }
    }

    pub fn name(&self) -> &RoomName {
        &self.name
    }

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
        let (next, changed) = {
            let mut state = self.state.lock().expect("room lock");
            let changed = state.apply(cmd, now_ms);
            if changed {
                state.rev += 1;
            }
            (state.clone(), changed)
        };
        self.publish();
        if let Some(snapshots) = self.snapshots.as_ref().filter(|_| changed) {
            snapshots.mark(&self.name);
        }
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
