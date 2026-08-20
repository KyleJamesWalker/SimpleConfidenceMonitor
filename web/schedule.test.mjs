// Run with: node --test web/schedule.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { nextClockTime } from './shared.js';

// 2026-01-02 at 12:00 local.
const NOON = new Date(2026, 0, 2, 12, 0, 0).getTime();

test('a time later today lands today', () => {
  const at = nextClockTime('14:30', NOON);
  const when = new Date(at);
  assert.equal(when.getHours(), 14);
  assert.equal(when.getMinutes(), 30);
  assert.equal(when.getDate(), 2);
});

test('a time already past lands tomorrow', () => {
  const when = new Date(nextClockTime('09:00', NOON));
  assert.equal(when.getHours(), 9);
  assert.equal(when.getDate(), 3);
});

test('the current minute counts as past', () => {
  const when = new Date(nextClockTime('12:00', NOON));
  assert.equal(when.getDate(), 3);
});

test('seconds are allowed', () => {
  const when = new Date(nextClockTime('14:30:15', NOON));
  assert.equal(when.getSeconds(), 15);
});

test('a time without a colon reads as an hour', () => {
  const when = new Date(nextClockTime('14', NOON));
  assert.equal(when.getHours(), 14);
  assert.equal(when.getMinutes(), 0);
});

test('refuses text that is not a clock time', () => {
  for (const bad of ['', '  ', 'soon', '25:00', '12:60', '-1:00', '12:00:00:00']) {
    assert.equal(nextClockTime(bad, NOON), null, `expected ${bad} to be refused`);
  }
});
