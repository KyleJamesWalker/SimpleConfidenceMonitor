# SimpleConfidenceMonitor: a single-binary speaker timer and confidence monitor

This document specifies version 1 of the server: the routes, the state model,
the sync protocol, the two views, and the milestone order.

## Summary

SimpleConfidenceMonitor is one Rust binary that serves a confidence monitor over
HTTP. An operator opens `/<room>/edit` on a laptop or tablet and drives a timer,
a message, and the screen appearance. The stage display opens `/<room>` in a
browser and follows in real time over a WebSocket. Rooms are named by whatever
string appears in the URL, and the server creates one on first request. The
build has no Node toolchain and no runtime dependencies: `cargo build --release`
produces a self-contained binary with the frontend embedded.

Version 1 covers one active timer per room. A rundown or cue list is a later
milestone, and the wire format leaves room for it.

## Background

Four products define what operators expect from this category.

| Product | What it contributes to the model |
|---|---|
| [stagetimer.io](https://stagetimer.io/features/) | Room per event, separate controller and viewer URLs, three signal colors, blackout and flash, HTTP API |
| [Ontime](https://docs.getontime.no/) | Rundown as the source of truth, role-specific views, WebSocket and OSC integration |
| CueTimer | Cue list, count-up and countdown, overtime in red, messages to the speaker |
| FreeShow | Stage view with next-item text, LAN remote control, output layouts |

The shared core across all four is small. It is a named room, one authoritative
timer, warning thresholds, a message overlay, and a viewer that renders nothing
but the show. Everything else in those products covers scheduling, media, or
hardware integration.

## Problem

A speaker timer for a small event has to satisfy four constraints at once.

1. The stage display must stay within a frame of the operator screen. Polling at
   one-second intervals drifts and stutters.
2. Setup happens minutes before doors open. Any step beyond "open a URL" gets
   skipped.
3. The venue network may have no internet. A hosted service is not an option.
4. The operator device is whatever is in the room. A tablet browser has to work.

A single binary with an embedded frontend and a server-authoritative timer meets
all four. Nothing installs on the display machine, and the only shared state is
a room name.

## Proposed design

### Routes

| Route | Purpose |
|---|---|
| `GET /` | Room picker. Enter a name, get the two links and a QR code for the viewer |
| `GET /<room>` | Viewer. The confidence monitor itself |
| `GET /<room>/edit` | Operator console. Requires the admin token |
| `GET /api/rooms/<room>` | Room state as JSON |
| `POST /api/rooms/<room>/cmd` | Apply one command. Requires the admin token |
| `GET /api/rooms/<room>/ws` | WebSocket. `?role=view` or `?role=edit` |
| `GET /healthz` | Liveness |
| `GET /assets/*` | Embedded CSS, JavaScript, and fonts |

The server reserves `api`, `assets`, and `healthz`, and rejects them as room
names. A room name is 1 to 64 characters from `[a-z0-9-_]` after lowercasing.
That rule keeps the name safe in a URL and safe as a snapshot filename.

### State model

One room holds the whole show state. The server is the only writer.

```rust
struct Room {
    rev: u64,                     // increments on every accepted command
    timer: Timer,
    message: Message,
    display: Display,
}

struct Timer {
    mode: Mode,                   // Countdown | CountUp | TimeOfDay
    duration_ms: u64,             // target for Countdown
    run: Run,                     // Stopped | Running { since_ms } | Paused
    elapsed_ms: u64,              // accrued across pauses
    warn_ms: u64,                 // amber threshold
    danger_ms: u64,               // red threshold
    on_expire: OnExpire,          // CountNegative | HoldAtZero
}

struct Message {
    text: String,                 // operator note to the speaker
    tone: Tone,                   // Neutral | Warn | Alert
    visible: bool,
    flash: bool,
}

struct Display {
    title: String,                // session or speaker name
    next_up: String,              // what follows this slot
    show_clock: bool,
    clock_24h: bool,
    show_progress: bool,
    blackout: bool,
    mirror: bool,                 // horizontal flip for teleprompter glass
    scale: u8,                    // 50 to 200 percent
}
```

Timer readout is a pure function of `(Timer, now)`. No task ticks the clock, so
the server holds no timing loop and the state machine tests without sleeping.

### Sync

The server owns the clock. Each state frame carries `server_time_ms` and the
timer anchor. On connect the client measures round-trip time over three pings
and computes a clock offset. It then renders from its own animation frame loop
and corrects against every incoming frame. The result is a smooth readout that
survives a browser throttling background tabs.

The server broadcasts the full room state on every accepted command through a
`tokio::sync::broadcast` channel. Full state costs under 512 bytes, so a diff
protocol buys nothing and costs reconnect correctness. Clients also receive a
keepalive frame every 15 seconds, which doubles as clock re-sync.

A viewer that loses the socket keeps counting from its last anchor, shows a
connection warning after 5 seconds, and reconnects with capped exponential
backoff. Losing the network mid-talk must not blank the stage display.

### Commands

The same JSON command envelope arrives over the WebSocket and over
`POST /api/rooms/<room>/cmd`, so Bitfocus Companion and a Stream Deck reach
every operator action with `curl`.

```json
{ "cmd": "start" }
{ "cmd": "pause" }
{ "cmd": "reset" }
{ "cmd": "set_duration", "ms": 900000 }
{ "cmd": "adjust", "ms": -30000 }
{ "cmd": "set_mode", "mode": "count_up" }
{ "cmd": "message", "text": "Wrap up", "tone": "warn", "visible": true }
{ "cmd": "flash" }
{ "cmd": "blackout", "on": true }
{ "cmd": "display", "title": "Keynote", "next_up": "Panel: Q&A" }
```

An unknown `cmd` returns `400` and leaves state untouched. The server replies
with the new `rev` so a caller can confirm the command landed.

### Views

**Viewer.** Black background, one large timer, and nothing that moves without
reason. The digits turn amber at `warn_ms` and red at `danger_ms`. Past zero the
readout goes negative with a leading minus, still red. A message overlay takes
the lower third. Title, next-up line, wall clock, and progress bar each hide
independently.

`blackout` cuts to black and keeps the socket open. `flash` strobes the screen
twice. `mirror` applies `scaleX(-1)` for a display behind teleprompter glass.
Query parameters override per-screen appearance, so `/keynote?clock=0&mirror=1`
differs from the stage feed without touching room state.

**Operator console.** The top half carries the timer readout, the transport
buttons, and duration entry. Duration entry offers quick chips at 5, 10, 15, 20,
and 30 minutes, plus `±30s` and `±1m` nudges. The bottom half carries the threshold fields, a message
composer with three tones, and a toggle for every `Display` field. A live viewer
preview sits in the corner. Keyboard
bindings cover the hot path: space starts and pauses, `r` resets, `b` toggles
blackout, `f` flashes, and `Enter` sends the message.

### Authentication

`--token <value>` gates every write. The console page and the mutating API check
it in three places, in order: an `Authorization: Bearer` header, a `token` query
parameter, and a `scm_token` cookie. A request that arrives with a valid query
parameter gets the cookie set, so an operator pastes the link once and navigates
freely afterward. Comparison uses `subtle::ConstantTimeEq`.

Started without `--token`, the server logs a warning at startup and leaves the
console open. Viewer routes never require the token, because a display machine
in a locked booth cannot type one.

### Persistence

State lives in memory. Given `--state-dir <path>`, the server writes
`<path>/<room>.json` on change, debounced by one second, through a temp file and
a rename. At startup it loads every JSON file in the directory. A running timer
reloads as paused at its last elapsed value, because a restart means the show
already stopped.

### Layout and stack

```
src/
  main.rs        # clap CLI, tracing, axum server
  room.rs        # Room, Timer, command application, pure readout math
  hub.rs         # room registry, broadcast channels, snapshot debounce
  routes.rs      # HTTP handlers and route table
  ws.rs          # socket upgrade, ping and pong, role handling
  auth.rs        # token extraction and constant-time compare
  persist.rs     # snapshot load and atomic save
web/
  viewer.html    viewer.js    viewer.css
  console.html   console.js   console.css
  shared.js      # clock offset, socket client with backoff
```

Dependencies: `axum`, `tokio`, `serde`, `serde_json`, `clap`, `tracing`,
`tracing-subscriber`, `rust-embed`, `subtle`. The frontend is hand-written
JavaScript with no framework and no build step.

### Testing

- Unit tests over `Timer` readout and command application at fixed timestamps.
  Cover the transitions, threshold boundaries, and both expiry behaviors.
- Route tests over the axum app: room name validation, token rejection, command
  round-trip, and reserved prefixes.
- One WebSocket test: connect, send a command, and assert both the reply and the
  broadcast to a second client.
- Snapshot round-trip test, including the running-to-paused reload rule.

## Alternatives considered

**Server-side tick loop.** A task that ticks each room every 100 milliseconds
and pushes the readout is simpler on the client. It scales with room count
rather than with events, and network jitter becomes visible stutter. Rejected.

**Server-Sent Events instead of WebSocket.** SSE covers the viewer, which only
reads. The console needs a return path, so the design would carry two transports
plus the same clock-offset code. Rejected.

**Svelte or React console.** A framework would help a rundown editor. For four
panels of controls it adds an npm build stage ahead of `cargo build`, which
contradicts the single-command build. Revisit alongside the rundown.

**Per-room PIN instead of one server token.** Per-room secrets suit a shared or
hosted server. This binary runs on a laptop on the venue network for one event,
where one token is one fewer thing to lose. The `auth.rs` boundary keeps the
upgrade cheap.

**Rundown in version 1.** A cue list is the largest feature in Ontime and
CueTimer, and it roughly doubles the state model and the console. Shipping the
single timer first proves the sync layer, which the rundown then reuses.

## Risks and open questions

- **Browser clock offset.** The offset estimate degrades on a congested Wi-Fi
  network. Mitigation is a median over three pings plus correction on every
  keepalive. Measure the residual drift on a real venue network before calling
  this done.
- **Room name typos.** A viewer at `/keynote` and a console at `/keynot` both
  succeed and never meet. The room picker issues both links together, and the
  console shows the connected viewer count so the operator sees zero.
- **Fullscreen and sleep.** A display machine that sleeps or shows a
  notification breaks the show. The viewer offers a fullscreen button and holds a
  Screen Wake Lock where the browser supports it.
- **Open question: multicast discovery.** Advertising the server over mDNS would
  remove the need to read an IP address aloud. Deferred until the core works.
- **Open question: audio cue on expiry.** Autoplay policy blocks sound until the
  viewer page receives a click. The fullscreen button may cover this already.

## Rollout

Milestones, each ending on a working binary.

1. **M1 Skeleton.** CLI, axum server, embedded assets, room registry, `/healthz`.
2. **M2 Timer core.** `Timer`, command application, readout math, and the unit
   tests. No frontend yet.
3. **M3 Sync.** WebSocket, broadcast, clock offset, and a viewer that shows a
   countdown driven from the console.
4. **M4 Show features.** Messages, tones, flash, blackout, thresholds, title and
   next-up, wall clock, progress bar, mirror, scale.
5. **M5 Operations.** Token auth, snapshot persistence, HTTP command API, room
   picker with QR code, and the README.
6. **M6 Rundown.** Cue list, next and previous, auto-advance, and running total
   against schedule. Out of scope for version 1.

Rollback is `git revert` and rebuild. The binary holds no external state beyond
an optional snapshot directory, and a stale snapshot deletes safely.
