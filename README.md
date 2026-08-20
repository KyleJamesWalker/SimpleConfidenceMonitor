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

A controller that can only issue a GET reaches the same commands through query
parameters:

```bash
curl 'http://localhost:8080/api/rooms/keynote/cmd?cmd=adjust&ms=-30000&token=<token>'
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
| `schedule_start` | `at_ms` | Starts the timer at that epoch millisecond. Omit `at_ms` to cancel |
| `aux_start`, `aux_pause`, `aux_reset` | | Transport for the second timer |
| `aux_set_duration` | `ms` | Sets the second timer |
| `aux_adjust` | `ms` | Adds or removes time on the second timer |
| `aux_set` | `label`, `visible` | Names the second timer and shows or hides it |
| `clear_room` | | Returns every part of the room to its defaults |
| `message` | `text`, `tone`, `visible` | Note to the speaker. Every field is optional |
| `send_preset` | `index` | Sends the quick message at that position |
| `set_cues` | `cues` | Replaces the running order. Each cue takes `title`, `speaker`, `duration_ms`, `notes` |
| `set_presets` | `presets` | Replaces the quick messages. Each carries `text` and `tone` |
| `flash` | | Flashes the viewer twice |
| `blackout` | `on` | Cuts the viewer to black |
| `display` | `title`, `next_up`, `show_clock`, `clock_24h`, `show_progress`, `mirror`, `scale`, `chime`, `show_speaker`, `show_notes` | Screen settings. Every field is optional |

`tone` is `neutral`, `warn` or `alert`. `scale` is a percent between 50 and 200.

### Other endpoints

| Method | Path | Returns |
|---|---|---|
| `GET` | `/api/rooms` | The names of the live rooms |
| `GET` | `/api/rooms/<room>` | Room state, the same shape the socket sends |
| `DELETE` | `/api/rooms/<room>` | Clears the room, drops it, and deletes its snapshot |
| `GET` | `/api/rooms/<room>/ws?role=view\|edit` | The live socket |
| `GET` | `/api/rooms/<room>/rundown.csv` | The running order as CSV |
| `GET` | `/api/rooms/<room>/rundown.json` | The running order as JSON |
| `GET` | `/api/rooms/<room>/cmd?cmd=<name>` | Runs one command from query parameters |
| `POST` | `/api/rooms/<room>/rundown` | Replaces the running order from CSV or JSON |
| `GET` | `/api/qr?text=<url>` | An SVG QR code |
| `GET` | `/healthz` | `ok` |

### The speaker and the note

A loaded cue carries a speaker and a note, and the viewer can show both. The
speaker sits beside the title and starts visible. The note sits under the next
up line and starts hidden, because a note often addresses the crew rather than
the room. `?speaker=0` and `?notes=1` set them per screen.

### Clearing a room

`clear_room` returns the timer, the message, the screen, the rundown, the
presets and the second timer to their defaults, and the console carries a
button for it. `rev` keeps climbing, so every connected screen sees the change.

`DELETE /api/rooms/<room>` goes further. It clears the room, drops it from the
registry, and deletes its snapshot. The picker offers this per room. A room
comes back empty the next time anyone opens it.

### The second timer

A room carries a second countdown beside the main one, for a break or a hard
stop. It runs on its own clock, so a break can count down while the session
timer keeps running. Give it a label, show it, and the viewer puts it under the
main readout. `?aux=0` hides it on one screen.

### Starting at a clock time

A cue can wait for the clock instead of a press. Arm a time on the console, or
post the epoch millisecond:

```bash
curl -X POST http://localhost:8080/api/rooms/keynote/cmd \
  -H 'content-type: application/json' \
  -d '{"cmd": "schedule_start", "at_ms": 1767225600000}'
```

The timer waits at the top and the viewer counts down to the start. Starting by
hand, resetting, or loading a cue all cancel a pending start. A time already
past starts within a fifth of a second.

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

`clock`, `progress`, `mirror`, `blackout`, `sound`, `aux`, `speaker` and
`notes` each take `1` or `0`. Use
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
| `--name` | the port | Name to advertise on the local network |
| `--no-mdns` | off | Stop advertising the server over mDNS |

Started without `--token`, the server logs a warning and leaves the console
open. Viewer routes never ask for the token, because a display machine cannot
type one.

The console takes the token from `?token=<value>` once and keeps it in a cookie.
Scripts send it as `Authorization: Bearer <value>`.

With `--state-dir`, the server writes each room to `<dir>/<room>.json` one
second after it settles. Rooms load again at startup, and a timer that was
running comes back paused at its saved elapsed time.

### Finding the server

The server advertises itself over mDNS as `_scm._tcp.local.`, so a phone or a
laptop on the same network can find it without anyone reading an IP address
aloud. `--name "Main Stage"` sets the name a browser shows. `--no-mdns` turns
the announcement off, and a network that blocks multicast logs a warning and
carries on.

```bash
dns-sd -B _scm._tcp local        # macOS
avahi-browse -r _scm._tcp        # Linux
```

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
