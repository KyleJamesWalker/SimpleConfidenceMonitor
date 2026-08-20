// Run with: node --test web/picker.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { normalizeRoomName, roomLinks } from './shared.js';

test('keeps a name the server already accepts', () => {
  assert.equal(normalizeRoomName('keynote'), 'keynote');
  assert.equal(normalizeRoomName('main_stage-2'), 'main_stage-2');
});

test('lowercases and trims', () => {
  assert.equal(normalizeRoomName('  KeyNote '), 'keynote');
});

test('replaces runs of unsupported characters with one dash', () => {
  assert.equal(normalizeRoomName('Main Stage'), 'main-stage');
  assert.equal(normalizeRoomName('room #1 (big)'), 'room-1-big');
  assert.equal(normalizeRoomName('a/b\\c'), 'a-b-c');
});

test('drops leading and trailing dashes', () => {
  assert.equal(normalizeRoomName('...keynote...'), 'keynote');
  assert.equal(normalizeRoomName('!!!'), '');
});

test('caps the name at the length the server allows', () => {
  assert.equal(normalizeRoomName('a'.repeat(100)).length, 64);
});

test('builds a viewer link that carries no token', () => {
  const links = roomLinks('http://192.168.1.20:8080', 'keynote', 's3cret');
  assert.equal(links.viewer, 'http://192.168.1.20:8080/keynote');
});

test('builds a console link that carries the token', () => {
  const links = roomLinks('http://192.168.1.20:8080', 'keynote', 's3cret');
  assert.equal(links.console, 'http://192.168.1.20:8080/keynote/edit?token=s3cret');
});

test('omits the query when there is no token', () => {
  const links = roomLinks('http://host', 'keynote', '');
  assert.equal(links.console, 'http://host/keynote/edit');
});

test('escapes a token with url characters', () => {
  const links = roomLinks('http://host', 'keynote', 'a b&c');
  assert.equal(links.console, 'http://host/keynote/edit?token=a%20b%26c');
});
