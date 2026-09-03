// Run with: node --test web/keys.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { targetOwnsKey } from './shared.js';

test('a text field owns every key', () => {
  for (const tag of ['INPUT', 'TEXTAREA', 'SELECT']) {
    assert.equal(targetOwnsKey(tag, ' '), true);
    assert.equal(targetOwnsKey(tag, 'b'), true);
  }
});

// Space on a focused rundown control used to start or pause the show while the
// button itself never fired.
test('a focused button owns the keys that activate it', () => {
  assert.equal(targetOwnsKey('BUTTON', ' '), true);
  assert.equal(targetOwnsKey('BUTTON', 'Enter'), true);
  assert.equal(targetOwnsKey('A', ' '), true);
  assert.equal(targetOwnsKey('A', 'Enter'), true);
});

test('a focused button leaves the letter shortcuts alone', () => {
  for (const key of ['b', 'f', 'n', 'p', 'r', 'g']) {
    assert.equal(targetOwnsKey('BUTTON', key), false);
  }
});

test('the page itself owns every key', () => {
  assert.equal(targetOwnsKey('BODY', ' '), false);
  assert.equal(targetOwnsKey('DIV', 'Enter'), false);
});
