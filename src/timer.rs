use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Countdown,
    CountUp,
    TimeOfDay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case", tag = "state")]
pub enum Run {
    #[default]
    Stopped,
    Running {
        since_ms: u64,
    },
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum OnExpire {
    #[default]
    CountNegative,
    HoldAtZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Normal,
    Warn,
    Danger,
    Expired,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct Readout {
    pub mode: Mode,
    /// Signed for a countdown past zero.
    pub value_ms: i64,
    pub duration_ms: u64,
    pub elapsed_ms: u64,
    pub phase: Phase,
    pub progress: f32,
    pub running: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Timer {
    pub mode: Mode,
    pub duration_ms: u64,
    pub run: Run,
    pub elapsed_ms: u64,
    pub warn_ms: u64,
    pub danger_ms: u64,
    pub on_expire: OnExpire,
    /// Wall clock time the autopilot starts this timer, if an operator armed one.
    #[serde(default)]
    pub start_at_ms: Option<u64>,
}

pub const DEFAULT_DURATION_MS: u64 = 15 * 60 * 1000;
pub const DEFAULT_WARN_MS: u64 = 3 * 60 * 1000;
pub const DEFAULT_DANGER_MS: u64 = 60 * 1000;

impl Default for Timer {
    fn default() -> Self {
        Self {
            mode: Mode::default(),
            duration_ms: DEFAULT_DURATION_MS,
            run: Run::default(),
            elapsed_ms: 0,
            warn_ms: DEFAULT_WARN_MS,
            danger_ms: DEFAULT_DANGER_MS,
            on_expire: OnExpire::default(),
            start_at_ms: None,
        }
    }
}

impl Timer {
    pub fn is_running(&self) -> bool {
        matches!(self.run, Run::Running { .. })
    }

    /// Elapsed run time, including the segment in progress.
    pub fn elapsed_at(&self, now_ms: u64) -> u64 {
        match self.run {
            Run::Running { since_ms } => self.elapsed_ms + now_ms.saturating_sub(since_ms),
            _ => self.elapsed_ms,
        }
    }

    pub fn readout(&self, now_ms: u64) -> Readout {
        let elapsed_ms = self.elapsed_at(now_ms);
        let remaining_ms = self.duration_ms as i64 - elapsed_ms as i64;
        let value_ms = match self.mode {
            Mode::Countdown if self.on_expire == OnExpire::HoldAtZero => remaining_ms.max(0),
            Mode::Countdown => remaining_ms,
            Mode::CountUp | Mode::TimeOfDay => elapsed_ms as i64,
        };
        Readout {
            mode: self.mode,
            value_ms,
            duration_ms: self.duration_ms,
            elapsed_ms,
            phase: self.phase(remaining_ms),
            progress: self.progress(elapsed_ms),
            running: self.is_running(),
        }
    }

    /// A zero threshold is off, not a threshold that fires at once.
    fn phase(&self, remaining_ms: i64) -> Phase {
        if self.mode == Mode::TimeOfDay || self.duration_ms == 0 {
            return Phase::Normal;
        }
        if remaining_ms <= 0 {
            Phase::Expired
        } else if self.danger_ms > 0 && remaining_ms <= self.danger_ms as i64 {
            Phase::Danger
        } else if self.warn_ms > 0 && remaining_ms <= self.warn_ms as i64 {
            Phase::Warn
        } else {
            Phase::Normal
        }
    }

    fn progress(&self, elapsed_ms: u64) -> f32 {
        if self.duration_ms == 0 {
            return 0.0;
        }
        (elapsed_ms as f32 / self.duration_ms as f32).clamp(0.0, 1.0)
    }

    pub fn start(&mut self, now_ms: u64) -> bool {
        let armed = self.start_at_ms.take().is_some();
        if self.is_running() {
            return armed;
        }
        self.run = Run::Running { since_ms: now_ms };
        true
    }

    pub fn pause(&mut self, now_ms: u64) -> bool {
        let Run::Running { since_ms } = self.run else {
            return false;
        };
        self.elapsed_ms += now_ms.saturating_sub(since_ms);
        self.run = Run::Paused;
        true
    }

    pub fn reset(&mut self) -> bool {
        let armed = self.start_at_ms.take().is_some();
        if self.run == Run::Stopped && self.elapsed_ms == 0 {
            return armed;
        }
        self.run = Run::Stopped;
        self.elapsed_ms = 0;
        true
    }

    /// Arms a wall clock start, or clears one. The timer waits at the top.
    pub fn schedule_start(&mut self, at_ms: Option<u64>) -> bool {
        let changed = self.start_at_ms != at_ms || self.is_running() || self.elapsed_ms != 0;
        self.start_at_ms = at_ms;
        if at_ms.is_some() {
            self.run = Run::Stopped;
            self.elapsed_ms = 0;
        }
        changed
    }

    pub fn set_duration(&mut self, ms: u64) -> bool {
        if self.duration_ms == ms {
            return false;
        }
        self.duration_ms = ms;
        true
    }

    /// Adds or removes time from the target. Never goes below zero.
    pub fn adjust(&mut self, delta_ms: i64) -> bool {
        let next = (self.duration_ms as i64).saturating_add(delta_ms).max(0) as u64;
        self.set_duration(next)
    }

    pub fn set_mode(&mut self, mode: Mode) -> bool {
        if self.mode == mode {
            return false;
        }
        self.mode = mode;
        self.run = Run::Stopped;
        self.elapsed_ms = 0;
        true
    }
}
