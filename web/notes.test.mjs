// Run with: node --test web/notes.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { currentNote } from './shared.js';

const MIN = 60_000;
const note = (minutes, text) => ({ at_ms: minutes * MIN, text });

// A 30 minute cue with notes at the start, 5 minutes in and 25 minutes in.
const notes = [note(0, 'Open with the demo'), note(5, 'Three pillars'), note(25, 'Wrap up')];
const PLANNED = 30 * MIN;

const at = (remainingMinutes, list = notes, planned = PLANNED) =>
  currentNote(list, planned, remainingMinutes * MIN)?.text;

test('no notes means nothing to read', () => {
  assert.equal(currentNote([], PLANNED, 10 * MIN), null);
  assert.equal(currentNote(undefined, PLANNED, 10 * MIN), null);
});

test('the opening note stands until the next one is due', () => {
  assert.equal(at(30), 'Open with the demo');
  assert.equal(at(26), 'Open with the demo');
});

test('a note starts at its time into the cue', () => {
  assert.equal(at(25), 'Three pillars', '5 minutes in leaves 25 remaining');
  assert.equal(at(6), 'Three pillars');
  assert.equal(at(5), 'Wrap up', '25 minutes in leaves 5 remaining');
});

test('the last note stays through overtime', () => {
  assert.equal(at(0), 'Wrap up');
  assert.equal(at(-4), 'Wrap up');
});

// The operator gives the speaker two more minutes. The notes are written as
// time in, but they are measured by time left, so they slide with the gift.
test('time added mid-cue slides the later notes', () => {
  // 4 minutes elapsed of 30: 26 left, still the opener.
  assert.equal(at(26), 'Open with the demo');
  // +2 minutes, so at the same moment 28 are left and the opener holds.
  assert.equal(at(28), 'Open with the demo');
  // The 5 minute note now waits for 25 remaining, which is 7 minutes elapsed.
  assert.equal(at(25), 'Three pillars');
});

test('time added before the start does not delay the opening note', () => {
  assert.equal(at(35), 'Open with the demo', 'more time left than the cue plans for');
});

test('a note beyond the cue length waits for overtime', () => {
  const late = [note(0, 'Open'), note(35, 'You are over')];
  assert.equal(at(1, late), 'Open');
  assert.equal(at(-5, late), 'You are over');
});

test('an unsorted list is read in time order', () => {
  const jumbled = [note(25, 'Wrap up'), note(0, 'Open with the demo'), note(5, 'Three pillars')];
  assert.equal(at(25, jumbled), 'Three pillars');
  assert.equal(at(2, jumbled), 'Wrap up');
});

test('an empty note is skipped', () => {
  const withBlank = [note(0, 'Open'), { at_ms: 5 * MIN, text: '' }];
  assert.equal(at(20, withBlank), 'Open');
});

// Count-up and time-of-day have no planned length to measure against.
test('a cue with no length shows the opening note', () => {
  assert.equal(currentNote(notes, 0, 0)?.text, 'Open with the demo');
});
