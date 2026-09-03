// Run with: node --test web/socket.test.mjs
import assert from 'node:assert/strict';
import { test } from 'node:test';

globalThis.location = { protocol: 'http:', host: 'stage.local:8080' };

const { RoomSocket, INITIAL_BACKOFF_MS, MAX_BACKOFF_MS, nextBackoffMs } = await import(
  './shared.js'
);

// Stands in for the browser's WebSocket: it records what it was told and lets
// a test decide when the handshake succeeds or the connection drops.
class FakeSocket {
  constructor(url) {
    this.url = url;
    this.readyState = 0;
    this.listeners = {};
    this.sent = [];
  }

  addEventListener(type, handler) {
    (this.listeners[type] ||= []).push(handler);
  }

  emit(type, event) {
    for (const handler of this.listeners[type] || []) handler(event);
  }

  send(text) {
    this.sent.push(text);
  }

  accept() {
    this.readyState = 1;
    this.emit('open');
  }

  drop() {
    this.readyState = 3;
    this.emit('close');
  }
}

// retry() awaits the auth probe, so let the microtasks settle.
const settle = () => new Promise((resolve) => setImmediate(resolve));

function harness({ role = 'edit', authStatus = 200 } = {}) {
  const sockets = [];
  const waits = [];
  const statuses = [];
  const probes = [];
  let refusals = 0;

  const client = new RoomSocket({
    room: 'keynote',
    role,
    onStatus: (status) => statuses.push(status),
    onRefused: () => {
      refusals += 1;
    },
    openSocket: (url) => {
      const socket = new FakeSocket(url);
      sockets.push(socket);
      return socket;
    },
    probeAuth: async () => {
      probes.push(true);
      return authStatus;
    },
    schedule: (run, wait) => {
      waits.push(wait);
      return { run, wait };
    },
  });

  return {
    client,
    sockets,
    waits,
    statuses,
    probes,
    last: () => sockets[sockets.length - 1],
    refusals: () => refusals,
  };
}

test('the socket opens on the room and the role', () => {
  const { last } = harness();
  assert.equal(last().url, 'ws://stage.local:8080/api/rooms/keynote/ws?role=edit');
});

test('a dropped socket reconnects after the first backoff', async () => {
  const world = harness();
  world.last().accept();
  world.last().drop();
  await settle();
  assert.deepEqual(world.waits, [INITIAL_BACKOFF_MS]);
  assert.equal(world.statuses.at(-1), 'offline');
});

test('the backoff doubles up to the cap', async () => {
  const world = harness();
  world.last().accept();
  for (let attempt = 0; attempt < 7; attempt += 1) {
    world.last().drop();
    await settle();
    world.client.connect();
  }
  assert.deepEqual(world.waits, [250, 500, 1000, 2000, 4000, 5000, 5000]);
  assert.equal(world.waits.at(-1), MAX_BACKOFF_MS);
});

test('a socket that opens again starts over from the first backoff', async () => {
  const world = harness();
  world.last().accept();
  world.last().drop();
  await settle();
  world.client.connect();
  world.last().accept();
  world.last().drop();
  await settle();
  assert.deepEqual(world.waits, [INITIAL_BACKOFF_MS, INITIAL_BACKOFF_MS]);
});

// The edit socket rides a cookie with a day on it. Past that the handshake is
// refused, and reconnecting forever would leave the console dead on `offline`.
test('a refused handshake reports the token rather than retrying', async () => {
  const world = harness({ authStatus: 401 });
  world.last().drop();
  await settle();
  assert.equal(world.statuses.at(-1), 'unauthorized');
  assert.equal(world.refusals(), 1);
  assert.deepEqual(world.waits, [], 'a refused token must not reconnect on backoff');
  assert.equal(world.sockets.length, 1);
});

test('a socket that opened and then dropped is treated as the network', async () => {
  const world = harness({ authStatus: 401 });
  world.last().accept();
  world.last().drop();
  await settle();
  assert.equal(world.statuses.at(-1), 'offline');
  assert.equal(world.refusals(), 0);
  assert.deepEqual(world.waits, [INITIAL_BACKOFF_MS]);
});

test('a viewer socket never asks about the token', async () => {
  const world = harness({ role: 'view', authStatus: 401 });
  world.last().drop();
  await settle();
  assert.deepEqual(world.probes, [], 'the viewer socket needs no token');
  assert.deepEqual(world.waits, [INITIAL_BACKOFF_MS]);
});

test('the backoff is capped doubling', () => {
  assert.equal(nextBackoffMs(250), 500);
  assert.equal(nextBackoffMs(4000), 5000);
  assert.equal(nextBackoffMs(5000), 5000);
});
