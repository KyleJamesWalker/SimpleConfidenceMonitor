// Run with: node --test web/chime.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { shouldChime } from './shared.js';

test('chimes when the timer crosses into overtime', () => {
  assert.equal(shouldChime('danger', 'expired'), true);
  assert.equal(shouldChime('normal', 'expired'), true);
});

test('does not chime while it stays in overtime', () => {
  assert.equal(shouldChime('expired', 'expired'), false);
});

test('does not chime on any other change', () => {
  assert.equal(shouldChime('normal', 'warn'), false);
  assert.equal(shouldChime('warn', 'danger'), false);
  assert.equal(shouldChime('expired', 'normal'), false);
});

test('does not chime on the first frame of an already expired timer', () => {
  assert.equal(shouldChime(null, 'expired'), false);
  assert.equal(shouldChime(undefined, 'expired'), false);
});
