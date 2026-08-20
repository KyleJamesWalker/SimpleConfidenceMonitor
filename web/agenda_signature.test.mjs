// Run with: node --test web/agenda_signature.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { agendaSignature } from './shared.js';

const row = (over = {}) => ({
  id: 1,
  index: 0,
  state: 'planned',
  startMs: 1000,
  endMs: 2000,
  title: 'Keynote',
  speaker: 'Alice',
  durationMs: 1000,
  ...over,
});

test('an unchanged table keeps its signature', () => {
  assert.equal(agendaSignature([row()]), agendaSignature([row()]));
});

// Each of these used to leave the table stale.
for (const [field, value] of [
  ['id', 2],
  ['state', 'active'],
  ['startMs', 9999],
  ['endMs', 9999],
  ['title', 'Panel'],
  ['speaker', 'Bob'],
  ['durationMs', 5000],
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
