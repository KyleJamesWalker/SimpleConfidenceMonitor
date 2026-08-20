// Run with: node --test web/rundown.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { rundownTotals } from './shared.js';

const MIN = 60_000;

const rundown = (active = null) => ({
  cues: [
    { id: 1, title: 'Welcome', duration_ms: 5 * MIN },
    { id: 2, title: 'Keynote', duration_ms: 30 * MIN },
    { id: 3, title: 'Panel', duration_ms: 20 * MIN },
  ],
  active,
  auto_advance: false,
});

test('an empty rundown totals nothing', () => {
  const totals = rundownTotals({ cues: [], active: null }, 0);
  assert.deepEqual(totals, {
    cueCount: 0,
    activeIndex: -1,
    totalMs: 0,
    remainingMs: 0,
    doneMs: 0,
  });
});

test('with no active cue the whole plan remains', () => {
  const totals = rundownTotals(rundown(), 0);
  assert.equal(totals.totalMs, 55 * MIN);
  assert.equal(totals.remainingMs, 55 * MIN);
  assert.equal(totals.doneMs, 0);
  assert.equal(totals.activeIndex, -1);
});

test('the active cue contributes only its remaining time', () => {
  const totals = rundownTotals(rundown(1), 2 * MIN);
  assert.equal(totals.remainingMs, 52 * MIN);
  assert.equal(totals.doneMs, 3 * MIN);
  assert.equal(totals.activeIndex, 0);
});

test('cues before the active one count as done', () => {
  const totals = rundownTotals(rundown(3), 20 * MIN);
  assert.equal(totals.remainingMs, 20 * MIN);
  assert.equal(totals.doneMs, 35 * MIN);
});

test('overtime on the active cue does not go below zero', () => {
  const totals = rundownTotals(rundown(1), -90_000);
  assert.equal(totals.remainingMs, 50 * MIN);
  assert.equal(totals.doneMs, 5 * MIN);
});

test('an active id that is gone falls back to the whole plan', () => {
  const totals = rundownTotals(rundown(99), 5 * MIN);
  assert.equal(totals.remainingMs, 55 * MIN);
  assert.equal(totals.activeIndex, -1);
});

test('counts the cues', () => {
  assert.equal(rundownTotals(rundown(2), MIN).cueCount, 3);
});
