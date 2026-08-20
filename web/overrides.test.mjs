// Run with: node --test web/overrides.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { screenOverrides } from './shared.js';

test('an empty query overrides nothing', () => {
  const over = screenOverrides('');
  for (const value of Object.values(over)) {
    assert.equal(value, null);
  }
});

test('a flag reads as true unless it is zero', () => {
  assert.equal(screenOverrides('?clock=0').clock, false);
  assert.equal(screenOverrides('?clock=1').clock, true);
  assert.equal(screenOverrides('?clock').clock, true);
  assert.equal(screenOverrides('?clock=yes').clock, true);
});

test('every screen flag is available', () => {
  const over = screenOverrides('?clock=0&progress=0&mirror=1&blackout=1&sound=1&aux=0&speaker=0&notes=1&next=0');
  assert.deepEqual(
    {
      clock: over.clock,
      progress: over.progress,
      mirror: over.mirror,
      blackout: over.blackout,
      sound: over.sound,
      aux: over.aux,
      speaker: over.speaker,
      notes: over.notes,
      next: over.next,
    },
    {
      clock: false,
      progress: false,
      mirror: true,
      blackout: true,
      sound: true,
      aux: false,
      speaker: false,
      notes: true,
      next: false,
    },
  );
});

test('scale takes a number', () => {
  assert.equal(screenOverrides('?scale=150').scale, 150);
  assert.equal(screenOverrides('?scale=87.6').scale, 88);
});

test('scale clamps to the range the room allows', () => {
  assert.equal(screenOverrides('?scale=5').scale, 50);
  assert.equal(screenOverrides('?scale=900').scale, 200);
});

test('scale that is not a number overrides nothing', () => {
  assert.equal(screenOverrides('?scale=big').scale, null);
  assert.equal(screenOverrides('?scale=').scale, null);
});

test('title replaces the text on this screen', () => {
  assert.equal(screenOverrides('?title=Room%20A').title, 'Room A');
});

test('an empty title blanks the line', () => {
  assert.equal(screenOverrides('?title=').title, '');
});

test('a title of zero stays the text zero', () => {
  assert.equal(screenOverrides('?title=0').title, '0');
});
