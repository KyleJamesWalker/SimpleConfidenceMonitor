// Run with: node --test web/agenda_signature.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { agendaSignature, projectAgenda } from './shared.js';

const MIN = 60_000;
// On a minute boundary, so a tick inside the minute cannot cross one.
const NOW = 1_700_000_040_000;

const row = (over = {}) => ({
  id: 1,
  index: 0,
  state: 'planned',
  startMs: NOW,
  endMs: NOW + 5 * MIN,
  title: 'Keynote',
  speaker: 'Alice',
  durationMs: 5 * MIN,
  ...over,
});

test('an unchanged table keeps its signature', () => {
  assert.equal(agendaSignature([row()]), agendaSignature([row()]));
});

// Each of these used to leave the table stale.
for (const [field, value] of [
  ['id', 2],
  ['state', 'active'],
  ['startMs', NOW + 9 * MIN],
  ['endMs', NOW + 90 * MIN],
  ['title', 'Panel'],
  ['speaker', 'Bob'],
  ['durationMs', 6 * MIN],
]) {
  test(`a change of ${field} repaints`, () => {
    assert.notEqual(
      agendaSignature([row()]),
      agendaSignature([row({ [field]: value })]),
      `${field} is missing from the signature`,
    );
  });
}

test('adding or removing a cue repaints', () => {
  assert.notEqual(agendaSignature([row()]), agendaSignature([row(), row({ id: 2 })]));
  assert.notEqual(agendaSignature([row()]), agendaSignature([]));
});

test('a row with no clock time keeps its signature', () => {
  const done = row({ state: 'done', startMs: null, endMs: null });
  assert.equal(agendaSignature([done]), agendaSignature([done]));
});

// The table prints start and end to the minute, and two consecutive
// projections are what the 500 ms tick actually feeds the signature.
const rundown = (active = null) => ({
  cues: [
    { id: 1, title: 'Welcome', speaker: 'Kyle', duration_ms: 5 * MIN },
    { id: 2, title: 'Keynote', speaker: 'Alice', duration_ms: 30 * MIN },
  ],
  active,
  auto_advance: false,
});

const signatureAt = (active, remainingMs, nowMs) =>
  agendaSignature(projectAgenda(rundown(active), remainingMs, nowMs));

test('a tick before the show does not rebuild the table', () => {
  assert.equal(
    signatureAt(null, 0, NOW),
    signatureAt(null, 0, NOW + 500),
    'with no cue active every projected time tracks the wall clock',
  );
});

test('a tick during a cue does not rebuild the table', () => {
  assert.equal(
    signatureAt(1, 3 * MIN, NOW),
    signatureAt(1, 3 * MIN - 500, NOW + 500),
    'the active row moves by half a second, which the table does not show',
  );
});

test('crossing a minute rebuilds the table', () => {
  assert.notEqual(signatureAt(null, 0, NOW), signatureAt(null, 0, NOW + MIN));
});
