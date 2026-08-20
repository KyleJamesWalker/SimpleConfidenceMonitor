# SimpleConfidenceMonitor architecture

One Rust binary serves every screen a session needs: a stage display, an
operator console, and a read-only agenda. Nothing else runs. The frontend is
compiled into the binary, so a venue laptop needs no toolchain, no database and
no internet.

## Data flow

A room is the unit of state. One `Room` holds everything about one session, and
the server is its only writer.

1. A request for `/<room>` or `/<room>/edit` creates the room if it is new, and
   returns an embedded HTML page.
2. The page opens a WebSocket at `/api/rooms/<room>/ws`, as `role=view` or
   `role=edit`.
3. An operator action becomes one JSON command, over that socket or over
   `POST /api/rooms/<room>/cmd`.
4. The room applies the command, increments `rev` when the state changed, and
   broadcasts the whole state to every subscriber.
5. Each browser renders from its own animation frame loop, using the timer
   anchor in the frame plus its estimate of the server clock.

A command that changes nothing leaves `rev` alone and wakes no screen. That is
why a repeated blackout or a resent preset costs nothing.

### The clock

The server owns the clock, and no task ticks it. A readout is a pure function of
`(Timer, now)`, so the state machine tests at fixed timestamps and the server
holds no timing loop.

Each client estimates how far the server clock sits ahead of its own. It sends
three pings on connect, charges half of each round trip to each direction, and
takes the median so one stalled response cannot drag the estimate. Every state
frame carries `server_time_ms`, and a keepalive every 15 seconds doubles as a
resynchronization.

Between frames the browser draws from its own clock plus that offset. Polling
cannot do this: at one-second intervals the digits stutter and drift. `make soak`
measures the residual, and holds inside two milliseconds on loopback.

A viewer that loses the socket keeps counting from its last anchor, warns after
five seconds, and reconnects with capped backoff. Losing the network mid-talk
must not blank the stage display.

## Components

### `src/room.rs`

`RoomState` and every command that acts on it: the timer, the message, the
display settings, the rundown, the presets and the auxiliary timer. `Room` wraps
that state in a mutex, owns the broadcast channel, counts connected clients, and
marks itself dirty for the snapshot writer.

Ids in a rundown are never reused. A console holding a stale cue list therefore
cannot load the cue that took a removed one's place.

### `src/timer.rs`

The timer state machine and its readout. Countdown, count-up and time-of-day; a
run that accrues elapsed time across pauses; amber and red thresholds where zero
means off; overtime that either counts negative or holds at zero. The auxiliary
timer reuses this type, so the two cannot drift apart in behavior.

### `src/hub.rs`

The room registry. A command or a socket creates a room, a read does not, and a
delete retires it: the room closes, its sockets end, and it takes no further
commands. The hub also restores rooms at startup and hands each new room the
snapshot writer.

### `src/routes.rs`

The axum router, every HTTP handler, and the auth gate on each. Also the QR
endpoint the picker uses.

### `src/ws.rs`

Socket upgrade, role handling, ping and pong, and the per-client forwarding loop.
A client subscribes before it joins, so it receives its own join frame and there
is one code path rather than two.

### `src/auth.rs`

Token extraction and a constant-time compare. Reads a bearer header, then a
query parameter, then a cookie.

### `src/persist.rs`

Snapshots. One JSON file per room, written through a temporary file and a
rename, debounced one second after a change.

### `src/autopilot.rs`

The only part of the server that watches a clock. A 200 millisecond scan starts
an armed room at its appointed time, and advances a rundown when a running cue
reaches zero. It reads nothing else, and the readout does not depend on it.

### `src/discovery.rs`

The optional mDNS advertisement, off unless `--mdns` says otherwise.

### `web/`

Hand-written HTML, CSS and JavaScript, compiled in by `rust-embed`. No
framework and no build step. `shared.js` holds the socket client, the clock
offset math and the readout, which the viewer, the console and the agenda all
use.

## Interfaces

| Surface | Contract |
|---|---|
| Screens | `/<room>`, `/<room>/edit`, `/<room>/agenda`, and the picker at `/` |
| Commands | One JSON envelope with a `cmd` field, over the socket, `POST .../cmd`, or `GET .../cmd?cmd=` |
| State | `GET /api/rooms/<room>` returns the same frame the socket sends |
| Rundown | CSV and JSON in both directions at `/api/rooms/<room>/rundown` |
| Snapshots | `<state-dir>/<room>.json`, holding `saved_at_ms` and the state |

[operations.md](operations.md) lists every command and endpoint. The serde types
in `src/room.rs` and `src/timer.rs` carry the authoritative shapes.

Saved state is forward compatible. Every field carries a serde default, so a
snapshot written by an older build loads rather than failing, and a missing
field takes its default.

## Configuration and deployment

Configuration is command-line only, and the binary holds no state beyond an
optional snapshot directory. See [operations.md](operations.md) for the flags
and [release.md](release.md) for what ships.

The readout math lives twice, in `src/timer.rs` and in `web/shared.js`, because
the browser has to draw between frames. Both sides assert the same cases, in
`tests/timer.rs` and `web/shared.test.mjs`. Change one and change the other.

## Decisions worth knowing

**Flash is an event, not a flag.** A boolean cannot express a second flash while
the first is still on screen. The command stamps `flash_at`, and a viewer
compares it against the value it last saw.

**Full state on the wire, not a diff.** A frame runs a few hundred bytes. A diff
protocol would buy nothing and would cost reconnect correctness.

**One server token, not a per-room secret.** This binary runs on a laptop for one
event, where one token is one fewer thing to lose. The `auth.rs` boundary keeps a
per-room upgrade cheap.

**No server tick for the readout.** A task pushing readouts every 100
milliseconds scales with room count rather than events. It also turns network
jitter into visible stutter. Auto-advance needs to notice zero, and takes a
narrow scan instead.

**No frontend framework.** A framework would help a larger rundown editor. For
these panels it would add an npm stage ahead of `cargo build`, which the
single-command build does not allow.

## Related references

- [operations.md](operations.md) for running it, and the full API
- [development.md](development.md) for the test suites and the layout
- [release.md](release.md) for the assets and the release flow
- The serde types in `src/room.rs`, which outrank this document
