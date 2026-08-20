use simple_confidence_monitor::timer::{Mode, OnExpire, Phase, Run, Timer};

const T0: u64 = 1_700_000_000_000;
const MIN: u64 = 60_000;

fn countdown(duration_ms: u64) -> Timer {
    Timer {
        duration_ms,
        ..Timer::default()
    }
}

#[test]
fn a_stopped_countdown_shows_the_full_duration() {
    let timer = countdown(10 * MIN);
    let out = timer.readout(T0);
    assert_eq!(out.value_ms, 10 * MIN as i64);
    assert_eq!(out.elapsed_ms, 0);
    assert!(!out.running);
    assert_eq!(out.phase, Phase::Normal);
}

#[test]
fn a_running_countdown_counts_down_with_the_clock() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert_eq!(
        timer.readout(T0 + 90_000).value_ms,
        (10 * MIN - 90_000) as i64
    );
    assert_eq!(timer.readout(T0 + 90_000).elapsed_ms, 90_000);
    assert!(timer.readout(T0).running);
}

#[test]
fn pause_freezes_the_readout() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    timer.pause(T0 + 60_000);
    assert_eq!(timer.readout(T0 + 60_000).value_ms, (9 * MIN) as i64);
    assert_eq!(timer.readout(T0 + 5 * MIN).value_ms, (9 * MIN) as i64);
    assert!(!timer.readout(T0 + 5 * MIN).running);
}

#[test]
fn start_after_a_pause_continues_from_the_pause_point() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    timer.pause(T0 + MIN);
    timer.start(T0 + 5 * MIN);
    assert_eq!(timer.readout(T0 + 6 * MIN).value_ms, (8 * MIN) as i64);
}

#[test]
fn start_on_a_running_timer_changes_nothing() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert!(!timer.start(T0 + MIN));
    assert_eq!(timer.readout(T0 + 2 * MIN).elapsed_ms, 2 * MIN);
}

#[test]
fn pause_on_a_stopped_timer_changes_nothing() {
    let mut timer = countdown(10 * MIN);
    assert!(!timer.pause(T0));
    assert_eq!(timer.run, Run::Stopped);
}

#[test]
fn reset_returns_to_the_full_duration() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert!(timer.reset());
    assert_eq!(timer.readout(T0 + 5 * MIN).value_ms, (10 * MIN) as i64);
    assert_eq!(timer.run, Run::Stopped);
}

#[test]
fn a_countdown_goes_negative_past_zero() {
    let mut timer = countdown(MIN);
    timer.start(T0);
    let out = timer.readout(T0 + 90_000);
    assert_eq!(out.value_ms, -30_000);
    assert_eq!(out.phase, Phase::Expired);
}

#[test]
fn hold_at_zero_stops_the_readout_at_zero() {
    let mut timer = Timer {
        duration_ms: MIN,
        on_expire: OnExpire::HoldAtZero,
        ..Timer::default()
    };
    timer.start(T0);
    let out = timer.readout(T0 + 90_000);
    assert_eq!(out.value_ms, 0);
    assert_eq!(out.phase, Phase::Expired);
}

#[test]
fn count_up_counts_away_from_zero() {
    let mut timer = Timer {
        mode: Mode::CountUp,
        duration_ms: 10 * MIN,
        ..Timer::default()
    };
    timer.start(T0);
    assert_eq!(timer.readout(T0 + 90_000).value_ms, 90_000);
}

#[test]
fn count_up_keeps_counting_past_the_target() {
    let mut timer = Timer {
        mode: Mode::CountUp,
        duration_ms: MIN,
        ..Timer::default()
    };
    timer.start(T0);
    let out = timer.readout(T0 + 90_000);
    assert_eq!(out.value_ms, 90_000);
    assert_eq!(out.phase, Phase::Expired);
}

#[test]
fn the_phase_turns_to_warn_at_the_warn_threshold() {
    let mut timer = Timer {
        duration_ms: 10 * MIN,
        warn_ms: 3 * MIN,
        danger_ms: MIN,
        ..Timer::default()
    };
    timer.start(T0);
    assert_eq!(timer.readout(T0 + 7 * MIN - 1).phase, Phase::Normal);
    assert_eq!(timer.readout(T0 + 7 * MIN).phase, Phase::Warn);
}

#[test]
fn the_phase_turns_to_danger_at_the_danger_threshold() {
    let mut timer = Timer {
        duration_ms: 10 * MIN,
        warn_ms: 3 * MIN,
        danger_ms: MIN,
        ..Timer::default()
    };
    timer.start(T0);
    assert_eq!(timer.readout(T0 + 9 * MIN - 1).phase, Phase::Warn);
    assert_eq!(timer.readout(T0 + 9 * MIN).phase, Phase::Danger);
}

#[test]
fn a_zero_threshold_never_fires() {
    let mut timer = Timer {
        duration_ms: 10 * MIN,
        warn_ms: 0,
        danger_ms: 0,
        ..Timer::default()
    };
    timer.start(T0);
    assert_eq!(timer.readout(T0 + 10 * MIN - 1).phase, Phase::Normal);
}

#[test]
fn time_of_day_mode_reports_no_phase_change() {
    let mut timer = Timer {
        mode: Mode::TimeOfDay,
        ..Timer::default()
    };
    timer.start(T0);
    assert_eq!(timer.readout(T0 + 60 * MIN).phase, Phase::Normal);
}

#[test]
fn progress_reports_the_fraction_elapsed() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert!((timer.readout(T0 + 5 * MIN).progress - 0.5).abs() < 1e-6);
    assert!((timer.readout(T0 + 20 * MIN).progress - 1.0).abs() < 1e-6);
    assert!((timer.readout(T0).progress).abs() < 1e-6);
}

#[test]
fn progress_stays_at_zero_without_a_duration() {
    let mut timer = countdown(0);
    timer.start(T0);
    assert_eq!(timer.readout(T0 + MIN).progress, 0.0);
}

#[test]
fn adjust_adds_time_to_the_target() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert!(timer.adjust(60_000));
    assert_eq!(timer.readout(T0).value_ms, (11 * MIN) as i64);
}

#[test]
fn adjust_removes_time_from_the_target() {
    let mut timer = countdown(10 * MIN);
    assert!(timer.adjust(-30_000));
    assert_eq!(timer.duration_ms, 10 * MIN - 30_000);
}

#[test]
fn adjust_never_takes_the_target_below_zero() {
    let mut timer = countdown(10_000);
    assert!(timer.adjust(-60_000));
    assert_eq!(timer.duration_ms, 0);
}

#[test]
fn set_duration_replaces_the_target() {
    let mut timer = countdown(10 * MIN);
    assert!(timer.set_duration(5 * MIN));
    assert_eq!(timer.readout(T0).value_ms, (5 * MIN) as i64);
    assert!(!timer.set_duration(5 * MIN));
}

#[test]
fn changing_the_mode_resets_the_run() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert!(timer.set_mode(Mode::CountUp));
    assert_eq!(timer.run, Run::Stopped);
    assert_eq!(timer.readout(T0 + MIN).elapsed_ms, 0);
    assert!(!timer.set_mode(Mode::CountUp));
}

#[test]
fn a_clock_that_moves_backwards_does_not_underflow() {
    let mut timer = countdown(10 * MIN);
    timer.start(T0);
    assert_eq!(timer.readout(T0 - 5_000).elapsed_ms, 0);
}
