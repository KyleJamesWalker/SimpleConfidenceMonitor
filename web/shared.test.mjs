// Run with: node --test web/
// These mirror tests/timer.rs. When one side changes, both must move together.
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { elapsedMs, formatClock, formatDuration, readout } from './shared.js';

const T0 = 1_700_000_000_000;
const MIN = 60_000;

const timer = (over = {}) => ({
  mode: 'countdown',
  duration_ms: 10 * MIN,
  run: { state: 'stopped' },
  elapsed_ms: 0,
  warn_ms: 3 * MIN,
  danger_ms: MIN,
  on_expire: 'count_negative',
  ...over,
});

const running = (since = T0, over = {}) =>
  timer({ run: { state: 'running', since_ms: since }, ...over });

test('a stopped countdown shows the full duration', () => {
  const out = readout(timer(), T0);
  assert.equal(out.valueMs, 10 * MIN);
  assert.equal(out.running, false);
  assert.equal(out.phase, 'normal');
});

test('a running countdown counts down with the clock', () => {
  const out = readout(running(), T0 + 90_000);
  assert.equal(out.valueMs, 10 * MIN - 90_000);
  assert.equal(out.elapsedMs, 90_000);
  assert.equal(out.running, true);
});

test('a paused timer holds its elapsed time', () => {
  const out = readout(timer({ run: { state: 'paused' }, elapsed_ms: MIN }), T0 + 5 * MIN);
  assert.equal(out.valueMs, 9 * MIN);
});

test('a countdown goes negative past zero', () => {
  const out = readout(running(T0, { duration_ms: MIN }), T0 + 90_000);
  assert.equal(out.valueMs, -30_000);
  assert.equal(out.phase, 'expired');
});

test('hold at zero stops the readout at zero', () => {
  const out = readout(
    running(T0, { duration_ms: MIN, on_expire: 'hold_at_zero' }),
    T0 + 90_000,
  );
  assert.equal(out.valueMs, 0);
  assert.equal(out.phase, 'expired');
});

test('count up counts away from zero', () => {
  const out = readout(running(T0, { mode: 'count_up' }), T0 + 90_000);
  assert.equal(out.valueMs, 90_000);
});

test('the phase turns to warn at the warn threshold', () => {
  assert.equal(readout(running(), T0 + 7 * MIN - 1).phase, 'normal');
  assert.equal(readout(running(), T0 + 7 * MIN).phase, 'warn');
});

test('the phase turns to danger at the danger threshold', () => {
  assert.equal(readout(running(), T0 + 9 * MIN - 1).phase, 'warn');
  assert.equal(readout(running(), T0 + 9 * MIN).phase, 'danger');
});

test('a zero threshold never fires', () => {
  const out = readout(running(T0, { warn_ms: 0, danger_ms: 0 }), T0 + 10 * MIN - 1);
  assert.equal(out.phase, 'normal');
});

test('time of day mode reports no phase change', () => {
  const out = readout(running(T0, { mode: 'time_of_day' }), T0 + 60 * MIN);
  assert.equal(out.phase, 'normal');
});

test('progress reports the fraction elapsed', () => {
  assert.equal(readout(running(), T0 + 5 * MIN).progress, 0.5);
  assert.equal(readout(running(), T0 + 20 * MIN).progress, 1);
  assert.equal(readout(running(), T0).progress, 0);
});

test('progress stays at zero without a duration', () => {
  assert.equal(readout(running(T0, { duration_ms: 0 }), T0 + MIN).progress, 0);
});

test('a clock that moves backwards does not go negative', () => {
  assert.equal(elapsedMs(running(), T0 - 5_000), 0);
});

test('formats under an hour as minutes and seconds', () => {
  assert.equal(formatDuration(0), '0:00');
  assert.equal(formatDuration(9_000), '0:09');
  assert.equal(formatDuration(90_000), '1:30');
  assert.equal(formatDuration(59 * MIN + 59_000), '59:59');
});

test('formats an hour and above with hours', () => {
  assert.equal(formatDuration(60 * MIN), '1:00:00');
  assert.equal(formatDuration(3_723_000), '1:02:03');
});

test('formats overtime with a leading minus', () => {
  assert.equal(formatDuration(-1_000), '-0:01');
  assert.equal(formatDuration(-90_000), '-1:30');
});

test('formats the wall clock in both conventions', () => {
  const at = new Date(2026, 0, 2, 15, 4, 5);
  assert.equal(formatClock(at, true), '15:04:05');
  assert.equal(formatClock(at, false), '3:04:05 PM');
  const midnight = new Date(2026, 0, 2, 0, 30, 0);
  assert.equal(formatClock(midnight, false), '12:30:00 AM');
});
