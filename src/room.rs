use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;

use crate::clock::now_ms;
use crate::persist::Snapshots;
use crate::timer::{Mode, OnExpire, Run, Timer};
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
    /// Sound a tone when the timer reaches zero.
    #[serde(default)]
    pub chime: bool,
    /// Show the speaker of the loaded cue.
    #[serde(default = "yes")]
    pub show_speaker: bool,
    /// Show the note on the loaded cue. A note may be for the crew.
    #[serde(default)]
    pub show_notes: bool,
}

fn yes() -> bool {
    true
}

/// Duration for a cue added without one.
pub const DEFAULT_CUE_MS: u64 = 5 * 60 * 1000;

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
            chime: false,
            show_speaker: true,
            show_notes: false,
        }
    }
}

/// A second countdown, for a break or a hard stop. It runs beside the main
/// timer and shares its readout math.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Aux {
    pub label: String,
    pub visible: bool,
    pub timer: Timer,
}

pub const DEFAULT_AUX_MS: u64 = 10 * 60 * 1000;

impl Default for Aux {
    fn default() -> Self {
        Self {
            label: String::new(),
            visible: false,
            timer: Timer {
                duration_ms: DEFAULT_AUX_MS,
                warn_ms: 0,
                danger_ms: 0,
                ..Timer::default()
            },
        }
    }
}

/// A message an operator sends with one press.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preset {
    pub text: String,
    #[serde(default)]
    pub tone: Tone,
}

pub const MAX_PRESETS: usize = 8;
pub const PRESET_TEXT_LIMIT: usize = 120;

fn default_presets() -> Vec<Preset> {
    [
        ("5 minutes left", Tone::Neutral),
        ("2 minutes left", Tone::Warn),
        ("Wrap up", Tone::Warn),
        ("Time is up", Tone::Alert),
        ("Slow down", Tone::Neutral),
    ]
    .into_iter()
    .map(|(text, tone)| Preset {
        text: text.to_string(),
        tone,
    })
    .collect()
}

/// One item in the running order.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cue {
    pub id: u64,
    pub title: String,
    pub speaker: String,
    pub duration_ms: u64,
    pub notes: String,
}

/// A cue as it arrives from an import or an API caller, before it has an id.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CueDraft {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub speaker: String,
    #[serde(
        default = "default_cue_ms",
        alias = "duration",
        deserialize_with = "clock_or_millis"
    )]
    pub duration_ms: u64,
    #[serde(default)]
    pub notes: String,
}

fn default_cue_ms() -> u64 {
    DEFAULT_CUE_MS
}

/// A duration arrives as "5:30" from a document, or as a plain number from a
/// caller that already counts milliseconds.
fn millis_from(value: serde_json::Value) -> Result<u64, String> {
    match value {
        serde_json::Value::String(text) => crate::rundown_io::parse_duration(&text)
            .ok_or_else(|| format!("{text} is not a duration")),
        serde_json::Value::Number(number) => number
            .as_u64()
            .map(minutes_or_millis)
            .ok_or_else(|| "a duration cannot be negative".to_string()),
        other => Err(format!("{other} is not a duration")),
    }
}

/// Under a second is nobody's timer, so a small number means minutes. That
/// matches the bare number a spreadsheet carries in its duration column.
fn minutes_or_millis(value: u64) -> u64 {
    if value > 0 && value < 1000 {
        value * 60_000
    } else {
        value
    }
}

fn clock_or_millis<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    let value = serde_json::Value::deserialize(deserializer)?;
    millis_from(value).map_err(D::Error::custom)
}

/// The same for a command field that may be absent.
fn optional_clock_or_millis<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;
    match Option::<serde_json::Value>::deserialize(deserializer)? {
        None | Some(serde_json::Value::Null) => Ok(None),
        Some(value) => millis_from(value).map(Some).map_err(D::Error::custom),
    }
}

/// The running order. An id is never reused, so a stale console cannot load
/// the wrong cue after a removal.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Rundown {
    pub cues: Vec<Cue>,
    pub active: Option<u64>,
    pub auto_advance: bool,
    next_id: u64,
}

impl Rundown {
    pub fn position_of(&self, id: u64) -> Option<usize> {
        self.cues.iter().position(|cue| cue.id == id)
    }

    pub fn cue(&self, id: u64) -> Option<&Cue> {
        self.cues.iter().find(|cue| cue.id == id)
    }

    pub fn active_position(&self) -> Option<usize> {
        self.active.and_then(|id| self.position_of(id))
    }

    fn take_id(&mut self) -> u64 {
        self.next_id += 1;
        self.next_id
    }
}

/// Everything a viewer needs to render the show.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoomState {
    /// Increments on every command that changes state.
    #[serde(default)]
    pub rev: u64,
    #[serde(default)]
    pub timer: Timer,
    #[serde(default)]
    pub message: Message,
    #[serde(default)]
    pub display: Display,
    #[serde(default)]
    pub rundown: Rundown,
    /// A snapshot from before presets existed comes back with the defaults.
    #[serde(default = "default_presets")]
    pub presets: Vec<Preset>,
    #[serde(default)]
    pub aux: Aux,
}

impl Default for RoomState {
    fn default() -> Self {
        Self {
            rev: 0,
            timer: Timer::default(),
            message: Message::default(),
            display: Display::default(),
            rundown: Rundown::default(),
            presets: default_presets(),
            aux: Aux::default(),
        }
    }
}

/// One operator action. The same envelope arrives over the socket and over HTTP.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum Command {
    Start,
    Pause,
    Reset,
    SetDuration {
        #[serde(alias = "duration", deserialize_with = "clock_or_millis")]
        ms: u64,
    },
    Adjust {
        ms: i64,
    },
    SetMode {
        mode: Mode,
    },
    SetThresholds {
        #[serde(alias = "warn", deserialize_with = "clock_or_millis")]
        warn_ms: u64,
        #[serde(alias = "danger", deserialize_with = "clock_or_millis")]
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
        chime: Option<bool>,
        show_speaker: Option<bool>,
        show_notes: Option<bool>,
    },
    AddCue {
        title: Option<String>,
        speaker: Option<String>,
        #[serde(
            default,
            alias = "duration",
            deserialize_with = "optional_clock_or_millis"
        )]
        duration_ms: Option<u64>,
        notes: Option<String>,
    },
    UpdateCue {
        id: u64,
        title: Option<String>,
        speaker: Option<String>,
        #[serde(
            default,
            alias = "duration",
            deserialize_with = "optional_clock_or_millis"
        )]
        duration_ms: Option<u64>,
        notes: Option<String>,
    },
    RemoveCue {
        id: u64,
    },
    MoveCue {
        id: u64,
        to: usize,
    },
    LoadCue {
        id: u64,
    },
    NextCue,
    PrevCue,
    /// Load the following cue and run it, which is what an operator wants when
    /// one talk ends and the next begins.
    NextAndStart,
    SetAutoAdvance {
        on: bool,
    },
    SendPreset {
        index: usize,
    },
    SetPresets {
        presets: Vec<Preset>,
    },
    SetCues {
        cues: Vec<CueDraft>,
    },
    ScheduleStart {
        #[serde(default)]
        at_ms: Option<u64>,
    },
    AuxStart,
    AuxPause,
    AuxReset,
    AuxSetDuration {
        #[serde(alias = "duration", deserialize_with = "clock_or_millis")]
        ms: u64,
    },
    AuxAdjust {
        ms: i64,
    },
    AuxSet {
        label: Option<String>,
        visible: Option<bool>,
    },
    /// Returns every part of the room to its default. rev keeps climbing.
    ClearRoom,
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
                chime,
                show_speaker,
                show_notes,
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
                if let Some(value) = chime {
                    self.display.chime = *value;
                }
                if let Some(value) = show_speaker {
                    self.display.show_speaker = *value;
                }
                if let Some(value) = show_notes {
                    self.display.show_notes = *value;
                }
                before != self.display
            }
            Command::AddCue {
                title,
                speaker,
                duration_ms,
                notes,
            } => {
                let id = self.rundown.take_id();
                self.rundown.cues.push(Cue {
                    id,
                    title: title.clone().unwrap_or_default(),
                    speaker: speaker.clone().unwrap_or_default(),
                    duration_ms: duration_ms.unwrap_or(DEFAULT_CUE_MS),
                    notes: notes.clone().unwrap_or_default(),
                });
                self.resync_screen(self.rundown.active);
                true
            }
            Command::UpdateCue {
                id,
                title,
                speaker,
                duration_ms,
                notes,
            } => {
                let Some(index) = self.rundown.position_of(*id) else {
                    return false;
                };
                let before = self.rundown.cues[index].clone();
                let cue = &mut self.rundown.cues[index];
                if let Some(title) = title {
                    cue.title = title.clone();
                }
                if let Some(speaker) = speaker {
                    cue.speaker = speaker.clone();
                }
                if let Some(duration_ms) = duration_ms {
                    cue.duration_ms = *duration_ms;
                }
                if let Some(notes) = notes {
                    cue.notes = notes.clone();
                }
                let changed = before != self.rundown.cues[index];
                if changed {
                    self.resync_screen(self.rundown.active);
                }
                changed
            }
            Command::RemoveCue { id } => {
                let Some(index) = self.rundown.position_of(*id) else {
                    return false;
                };
                let had_active = self.rundown.active;
                self.rundown.cues.remove(index);
                if had_active == Some(*id) {
                    self.rundown.active = None;
                }
                self.resync_screen(had_active);
                true
            }
            Command::MoveCue { id, to } => {
                let Some(from) = self.rundown.position_of(*id) else {
                    return false;
                };
                let target = (*to).min(self.rundown.cues.len().saturating_sub(1));
                if from == target {
                    return false;
                }
                let cue = self.rundown.cues.remove(from);
                self.rundown.cues.insert(target, cue);
                self.resync_screen(self.rundown.active);
                true
            }
            Command::LoadCue { id } => self.load_cue(*id, now_ms),
            Command::NextCue => self.step(1, now_ms),
            Command::PrevCue => self.step(-1, now_ms),
            Command::NextAndStart => {
                let moved = self.step(1, now_ms);
                if moved {
                    self.timer.start(now_ms);
                }
                moved
            }
            Command::SetAutoAdvance { on } => {
                let changed = self.rundown.auto_advance != *on;
                self.rundown.auto_advance = *on;
                changed
            }
            Command::ScheduleStart { at_ms } => self.timer.schedule_start(*at_ms),
            Command::ClearRoom => {
                let fresh = RoomState::default();
                let changed = self.timer != fresh.timer
                    || self.message != fresh.message
                    || self.display != fresh.display
                    || self.rundown != fresh.rundown
                    || self.presets != fresh.presets
                    || self.aux != fresh.aux;
                let rev = self.rev;
                *self = fresh;
                self.rev = rev;
                changed
            }
            Command::AuxStart => self.aux.timer.start(now_ms),
            Command::AuxPause => self.aux.timer.pause(now_ms),
            Command::AuxReset => self.aux.timer.reset(),
            Command::AuxSetDuration { ms } => self.aux.timer.set_duration(*ms),
            Command::AuxAdjust { ms } => self.aux.timer.adjust(*ms),
            Command::AuxSet { label, visible } => {
                let before = self.aux.clone();
                if let Some(label) = label {
                    self.aux.label = label.clone();
                }
                if let Some(visible) = visible {
                    self.aux.visible = *visible;
                }
                before != self.aux
            }
            Command::SetCues { cues } => {
                self.rundown.cues = cues
                    .iter()
                    .map(|draft| Cue {
                        id: self.rundown.take_id(),
                        title: draft.title.clone(),
                        speaker: draft.speaker.clone(),
                        duration_ms: draft.duration_ms,
                        notes: draft.notes.clone(),
                    })
                    .collect();
                let had_active = self.rundown.active;
                self.resync_screen(had_active);
                true
            }
            Command::SendPreset { index } => {
                let Some(preset) = self.presets.get(*index).cloned() else {
                    return false;
                };
                self.apply(
                    &Command::Message {
                        text: Some(preset.text),
                        tone: Some(preset.tone),
                        visible: Some(true),
                    },
                    now_ms,
                )
            }
            Command::SetPresets { presets } => {
                let next: Vec<Preset> = presets
                    .iter()
                    .filter(|preset| !preset.text.trim().is_empty())
                    .take(MAX_PRESETS)
                    .map(|preset| Preset {
                        text: preset.text.trim().chars().take(PRESET_TEXT_LIMIT).collect(),
                        tone: preset.tone,
                    })
                    .collect();
                let changed = self.presets != next;
                self.presets = next;
                changed
            }
        }
    }

    /// Points the timer and the screen at one cue, and starts it from zero.
    pub fn load_cue(&mut self, id: u64, _now_ms: u64) -> bool {
        if self.rundown.position_of(id).is_none() {
            return false;
        }
        self.rundown.active = Some(id);
        self.point_at_active_cue();
        self.timer.run = Run::Stopped;
        self.timer.elapsed_ms = 0;
        self.timer.start_at_ms = None;
        true
    }

    /// Keeps the screen in step with the running order. While a cue is loaded
    /// the title and the next-up line come from it, so any edit to the list
    /// refreshes them. Losing that cue clears both, since nobody typed them.
    fn resync_screen(&mut self, had_active: Option<u64>) {
        if self.rundown.active_position().is_some() {
            self.point_at_active_cue();
        } else if had_active.is_some() {
            self.rundown.active = None;
            self.display.title.clear();
            self.display.next_up.clear();
        }
    }

    /// Copies the active cue onto the screen and the timer target, and leaves
    /// the transport alone. Editing the cue on air must not stop the clock.
    fn point_at_active_cue(&mut self) {
        let Some(index) = self.rundown.active_position() else {
            return;
        };
        let cue = self.rundown.cues[index].clone();
        self.display.next_up = self
            .rundown
            .cues
            .get(index + 1)
            .map(|next| next.title.clone())
            .unwrap_or_default();
        self.display.title = cue.title;
        self.timer.duration_ms = cue.duration_ms;
    }

    /// Moves the active cue by one step. Stops at either end.
    fn step(&mut self, delta: i64, now_ms: u64) -> bool {
        if self.rundown.cues.is_empty() {
            return false;
        }
        let target = match self.rundown.active_position() {
            None => 0,
            Some(current) => {
                let next = current as i64 + delta;
                if next < 0 || next as usize >= self.rundown.cues.len() {
                    return false;
                }
                next as usize
            }
        };
        let id = self.rundown.cues[target].id;
        self.load_cue(id, now_ms)
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
    closed: AtomicBool,
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
            closed: AtomicBool::new(false),
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

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::Relaxed)
    }

    /// Retires the room. Sockets holding it end, and it takes no more commands,
    /// so a client that was mid-session cannot drive a room nobody can reach.
    pub fn close(&self) {
        self.closed.store(true, Ordering::Relaxed);
        self.publish();
    }

    /// Applies a command, tells every client, and returns the new state.
    pub fn apply(&self, cmd: &Command, now_ms: u64) -> RoomState {
        self.apply_all(std::slice::from_ref(cmd), now_ms)
    }

    /// Applies several commands under one lock and publishes once. A client
    /// never sees the room between two halves of one operator action.
    pub fn apply_all(&self, cmds: &[Command], now_ms: u64) -> RoomState {
        if self.is_closed() {
            return self.snapshot();
        }
        let (next, changed) = {
            let mut state = self.state.lock().expect("room lock");
            let mut changed = false;
            for cmd in cmds {
                if state.apply(cmd, now_ms) {
                    state.rev += 1;
                    changed = true;
                }
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
