// Run with: node --test web/keys.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { targetOwnsKey } from './shared.js';

test('a text field owns every key, focused however it got there', () => {
  for (const tag of ['INPUT', 'TEXTAREA', 'SELECT']) {
    assert.equal(targetOwnsKey(tag, ' ', false), true);
    assert.equal(targetOwnsKey(tag, 'b', false), true);
  }
});

// Space on a tabbed-to rundown control used to start or pause the show while
// the button itself never fired.
test('a control the operator tabbed to owns the keys that activate it', () => {
  assert.equal(targetOwnsKey('BUTTON', ' ', true), true);
  assert.equal(targetOwnsKey('BUTTON', 'Enter', true), true);
  assert.equal(targetOwnsKey('A', ' ', true), true);
  assert.equal(targetOwnsKey('A', 'Enter', true), true);
});

// A click leaves the button focused, and Space is the transport's key. Pausing
// the show after pressing Blackout must not toggle Blackout again.
test('a clicked button gives Space back to the transport', () => {
  assert.equal(targetOwnsKey('BUTTON', ' ', false), false);
  assert.equal(targetOwnsKey('BUTTON', 'Enter', false), false);
  assert.equal(targetOwnsKey('A', ' ', false), false);
});

test('a focused button leaves the letter shortcuts alone', () => {
  for (const key of ['b', 'f', 'n', 'p', 'r', 'g']) {
    assert.equal(targetOwnsKey('BUTTON', key, true), false);
  }
});

test('the page itself owns no key', () => {
  assert.equal(targetOwnsKey('BODY', ' ', true), false);
  assert.equal(targetOwnsKey('DIV', 'Enter', true), false);
});
