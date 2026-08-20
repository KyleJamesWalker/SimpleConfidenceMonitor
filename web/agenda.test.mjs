// Run with: node --test web/agenda.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { projectAgenda } from './shared.js';

const MIN = 60_000;
const NOW = 1_700_000_000_000;

const rundown = (active = null) => ({
  cues: [
    { id: 1, title: 'Welcome', speaker: 'Kyle', duration_ms: 5 * MIN },
    { id: 2, title: 'Keynote', speaker: 'Alice', duration_ms: 30 * MIN },
    { id: 3, title: 'Panel', speaker: '', duration_ms: 20 * MIN },
  ],
  active,
  auto_advance: false,
});

test('an empty rundown projects nothing', () => {
  assert.deepEqual(projectAgenda({ cues: [], active: null }, 0, NOW), []);
});

test('with nothing loaded every cue chains from now', () => {
  const rows = projectAgenda(rundown(), 0, NOW);
  assert.deepEqual(
    rows.map((row) => row.state),
    ['planned', 'planned', 'planned'],
  );
  assert.equal(rows[0].startMs, NOW);
  assert.equal(rows[0].endMs, NOW + 5 * MIN);
  assert.equal(rows[1].startMs, NOW + 5 * MIN);
  assert.equal(rows[2].endMs, NOW + 55 * MIN);
});

test('the active cue anchors on the clock', () => {
  const rows = projectAgenda(rundown(2), 10 * MIN, NOW);
  assert.equal(rows[1].state, 'active');
  assert.equal(rows[1].startMs, NOW - 20 * MIN);
  assert.equal(rows[1].endMs, NOW + 10 * MIN);
});

test('cues before the active one read as done', () => {
  const rows = projectAgenda(rundown(2), 10 * MIN, NOW);
  assert.equal(rows[0].state, 'done');
  assert.equal(rows[0].startMs, null);
  assert.equal(rows[0].endMs, null);
});

test('cues after the active one chain from its end', () => {
  const rows = projectAgenda(rundown(2), 10 * MIN, NOW);
  assert.equal(rows[2].state, 'planned');
  assert.equal(rows[2].startMs, NOW + 10 * MIN);
  assert.equal(rows[2].endMs, NOW + 30 * MIN);
});

test('an overrunning cue chains the rest from now, not from the past', () => {
  const rows = projectAgenda(rundown(2), -3 * MIN, NOW);
  assert.equal(rows[1].endMs, NOW - 3 * MIN);
  assert.equal(rows[2].startMs, NOW);
  assert.equal(rows[2].endMs, NOW + 20 * MIN);
});

test('an active id that is gone falls back to planning from now', () => {
  const rows = projectAgenda(rundown(99), 5 * MIN, NOW);
  assert.equal(rows[0].state, 'planned');
  assert.equal(rows[0].startMs, NOW);
});

test('each row carries what the page shows', () => {
  const row = projectAgenda(rundown(1), MIN, NOW)[0];
  assert.equal(row.id, 1);
  assert.equal(row.index, 0);
  assert.equal(row.title, 'Welcome');
  assert.equal(row.speaker, 'Kyle');
  assert.equal(row.durationMs, 5 * MIN);
});
