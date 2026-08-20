// Run with: node --test web/offset.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

import { medianOffset, offsetSample } from './shared.js';

// A sample: when the client sent the ping, when it saw the pong, and the
// server clock the pong carried.
const sample = (sentMs, receivedMs, serverMs) => ({ sentMs, receivedMs, serverMs });

test('a symmetric round trip with matching clocks reads zero', () => {
  assert.equal(offsetSample(sample(1000, 1020, 1010)), 0);
});

test('a server ahead of the client reads the difference', () => {
  assert.equal(offsetSample(sample(1000, 1020, 1510)), 500);
});

test('a server behind the client reads a negative offset', () => {
  assert.equal(offsetSample(sample(1000, 1020, 510)), -500);
});

test('half the round trip is charged to each direction', () => {
  // 100ms round trip, server stamped at the midpoint, so no offset.
  assert.equal(offsetSample(sample(0, 100, 50)), 0);
});

test('the median ignores one slow sample', () => {
  const samples = [
    sample(0, 20, 10),
    sample(100, 120, 110),
    // A 400ms stall on the way back puts this estimate 190ms out.
    sample(200, 600, 210),
  ];
  assert.equal(medianOffset(samples), 0);
});

test('the median of an even count takes the lower middle', () => {
  const samples = [sample(0, 20, 10), sample(0, 20, 110)];
  assert.equal(medianOffset(samples), 0);
});

test('no samples means no correction', () => {
  assert.equal(medianOffset([]), 0);
});

test('jitter around a real offset stays close to it', () => {
  const samples = [];
  for (let i = 0; i < 9; i += 1) {
    const rtt = 20 + (i % 3) * 40;
    const sent = i * 1000;
    // The server clock runs 250ms ahead of this client.
    samples.push(sample(sent, sent + rtt, sent + rtt / 2 + 250));
  }
  assert.equal(medianOffset(samples), 250);
});
