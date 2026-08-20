// Run with: node --test web/active_cue.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { activeCue } from './shared.js';

const cues = [
  { id: 1, title: 'Welcome', speaker: 'Kyle', notes: 'mention wifi', duration_ms: 1 },
  { id: 2, title: 'Keynote', speaker: 'Alice', notes: '', duration_ms: 1 },
];

test('finds the loaded cue', () => {
  assert.equal(activeCue({ cues, active: 2 }).title, 'Keynote');
});

test('reports nothing when no cue is loaded', () => {
  assert.equal(activeCue({ cues, active: null }), null);
});

test('reports nothing when the loaded id is gone', () => {
  assert.equal(activeCue({ cues, active: 99 }), null);
});

test('reports nothing for an empty rundown', () => {
  assert.equal(activeCue({ cues: [], active: null }), null);
  assert.equal(activeCue({}), null);
});
