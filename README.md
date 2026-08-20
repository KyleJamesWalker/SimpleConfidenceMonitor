# SimpleConfidenceMonitor

A speaker timer and confidence monitor served from one Rust binary. An operator
drives the timer from a phone, tablet or laptop, and the stage display follows
in real time in any browser.

## Overview

Give a room a name. The stage display opens `/<room>`, the operator opens
`/<room>/edit`, and both stay in step over a WebSocket. The server creates a
room on first request, so setup is one URL and no configuration.

The server holds the clock, and each browser measures its own offset from it.
The digits stay smooth between state frames, and a display that loses the
network keeps counting from its last anchor.

Everything ships inside the binary. There is no Node toolchain, no database and
no internet dependency, which suits a laptop on a venue network.

## Requirements

- Rust 1.94 or later to build
- Any current browser for the two screens
- Node 20 or later to run the JavaScript tests, which is optional

## Setup

```bash
cargo build --release
./target/release/simple-confidence-monitor --port 8080
```

The log prints the address to open. Copy the room links from the picker at `/`,
which also shows a QR code for the viewer URL.

## Usage

Start the server, then open two screens for a room named `keynote`:

| Screen | URL | Needs the token |
|---|---|---|
| Stage display | `http://<host>:8080/keynote` | no |
| Operator console | `http://<host>:8080/keynote/edit` | yes |
| Agenda | `http://<host>:8080/keynote/agenda` | no |

The console carries the timer, the message to the speaker, and the screen
controls. A row of quick messages sits above the message box, so the common
notes take one press. Replace them per room with `set_presets`. Hot keys cover the fast path: space starts and pauses, `r` resets,
`b` toggles blackout, `f` flashes the screen.

The agenda is a read-only page for backstage and for the speakers. It lists the
rundown with a projected clock time per cue, marks what is on now, and strikes
through what is done. Times follow the running cue, so a session that overruns
moves every later cue with it.

Every operator action is also an HTTP call, so Bitfocus Companion, a Stream
Deck or a shell script can drive the show:

```bash
curl -X POST http://localhost:8080/api/rooms/keynote/cmd \
  -H 'content-type: application/json' \
  -H 'authorization: Bearer <token>' \
  -d '{"cmd": "set_duration", "ms": 900000}'
```

### Commands

Each command is a JSON object with a `cmd` field. The reply is the new room
state, so a caller can confirm the `rev` moved.

| Command | Fields | Effect |
|---|---|---|
| `start` | | Starts or resumes the timer |
| `pause` | | Holds the readout and keeps the elapsed time |
| `reset` | | Back to the full duration, stopped |
| `set_duration` | `ms` | Sets the target |
| `adjust` | `ms` | Adds or removes time, never below zero |
| `set_mode` | `mode` | `countdown`, `count_up` or `time_of_day` |
| `set_thresholds` | `warn_ms`, `danger_ms` | Amber and red points. Zero turns one off |
| `set_on_expire` | `on_expire` | `count_negative` or `hold_at_zero` |
| `message` | `text`, `tone`, `visible` | Note to the speaker. Every field is optional |
| `send_preset` | `index` | Sends the quick message at that position |
| `set_cues` | `cues` | Replaces the running order. Each cue takes `title`, `speaker`, `duration_ms`, `notes` |
| `set_presets` | `presets` | Replaces the quick messages. Each carries `text` and `tone` |
| `flash` | | Flashes the viewer twice |
| `blackout` | `on` | Cuts the viewer to black |
| `display` | `title`, `next_up`, `show_clock`, `clock_24h`, `show_progress`, `mirror`, `scale`, `chime` | Screen settings. Every field is optional |

`tone` is `neutral`, `warn` or `alert`. `scale` is a percent between 50 and 200.

### Other endpoints

| Method | Path | Returns |
|---|---|---|
| `GET` | `/api/rooms` | The names of the live rooms |
| `GET` | `/api/rooms/<room>` | Room state, the same shape the socket sends |
| `GET` | `/api/rooms/<room>/ws?role=view\|edit` | The live socket |
| `GET` | `/api/rooms/<room>/rundown.csv` | The running order as CSV |
| `GET` | `/api/rooms/<room>/rundown.json` | The running order as JSON |
| `POST` | `/api/rooms/<room>/rundown` | Replaces the running order from CSV or JSON |
| `GET` | `/api/qr?text=<url>` | An SVG QR code |
| `GET` | `/healthz` | `ok` |

### Importing a running order

A rundown can come from a spreadsheet. Export the CSV, edit it, and post it
back:

```bash
curl http://localhost:8080/api/rooms/keynote/rundown.csv -o rundown.csv
curl -X POST http://localhost:8080/api/rooms/keynote/rundown \
  -H 'content-type: text/csv' --data-binary @rundown.csv
```

The columns are `title`, `speaker`, `duration` and `notes`. A header row names
them in any order, and common spellings map onto the same column, so `cue`,
`presenter` and `length` work too. A duration takes minutes, `mm:ss` or
`hh:mm:ss`, and an empty one falls back to five minutes. A row without a title
is an error, and a refused import leaves the running order alone. The console
carries the same import and export next to the cue list.

### Per-screen overrides

A query parameter overrides one setting for one screen, which lets a booth
monitor differ from the stage feed without touching the room:

```
/keynote?clock=0&progress=0&mirror=1
```

`clock`, `progress`, `mirror`, `blackout` and `sound` each take `1` or `0`. Use
`mirror` for a display behind teleprompter glass.

`chime` sounds a tone when the timer reaches zero. It starts off, and one screen
can carry it with `?sound=1` while the rest stay quiet. A browser blocks sound
until someone interacts with the page, so a viewer with the chime on shows a
button to tap once. Only the crossing into overtime rings, so a screen that
joins late stays silent.

## Configuration

| Flag | Default | Controls |
|---|---|---|
| `--port` | `8080` | Listening port |
| `--bind` | `0.0.0.0` | Listening address. The default lets the stage display reach the server |
| `--token` | none | Token required to open the console and to send commands. Also read from `SCM_TOKEN` |
| `--state-dir` | none | Directory for room snapshots. Without it, state stays in memory |

Started without `--token`, the server logs a warning and leaves the console
open. Viewer routes never ask for the token, because a display machine cannot
type one.

The console takes the token from `?token=<value>` once and keeps it in a cookie.
Scripts send it as `Authorization: Bearer <value>`.

With `--state-dir`, the server writes each room to `<dir>/<room>.json` one
second after it settles. Rooms load again at startup, and a timer that was
running comes back paused at its saved elapsed time.

## Development

```bash
make test     # cargo test, then the JavaScript tests
make lint     # clippy with warnings denied, and a format check
make run      # debug build on port 8080
```

`web/` holds the frontend, compiled into the binary by `rust-embed`. The timer
readout math lives twice: `src/timer.rs` renders the truth, and
`web/shared.js` repeats it so a browser can draw between frames. Both sides
assert the same cases, in `tests/timer.rs` and `web/shared.test.mjs`. Change one
and change the other.

`SPEC.md` records the design, the alternatives that lost, and the milestone
order.
