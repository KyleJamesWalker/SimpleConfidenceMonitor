# Developing

How to build it, how to test it, and the two rules that keep the frontend honest.

## Requirements

- Rust 1.94 or later
- Node 20 or later, for the JavaScript tests only
- No other toolchain. There is no bundler, no package manager and no build step
  for the frontend

## The loop

```bash
make test     # cargo test, then the JavaScript tests
make lint     # clippy with warnings denied, and a format check
make run      # debug build on port 8080
make soak     # measure clock drift for ten seconds
make build    # release binary
```

For local work, bind loopback rather than every interface:

```bash
cargo run -- --port 8080 --bind 127.0.0.1
```

## Layout

```
src/
  main.rs        clap CLI, tracing, startup, the autopilot task
  lib.rs         module list
  room.rs        RoomState, every command, the rundown and the presets
  timer.rs       the timer state machine and its readout
  hub.rs         room registry, restore, removal
  routes.rs      axum router, HTTP handlers, the auth gate, QR
  ws.rs          socket upgrade, roles, ping and pong
  wire.rs        the client and server frame types
  auth.rs        token extraction and constant-time compare
  persist.rs     snapshot load, atomic save, debounced writer
  autopilot.rs   armed starts and auto advance
  discovery.rs   the optional mDNS advertisement
  rundown_io.rs  CSV parsing and writing
  clock.rs       wall clock in epoch milliseconds
  assets.rs      the embedded web directory
web/
  viewer.html    viewer.css    viewer.js
  console.html   console.css   console.js
  agenda.html    agenda.css    agenda.js
  picker.html    picker.css    picker.js
  shared.js      socket client, clock offset, readout, formatting
  *.test.mjs     the node test suites
tests/           one file per area, all integration level
```

## Two rules

**The readout math lives twice.** `src/timer.rs` is the truth, and
`web/shared.js` repeats it so a browser can draw between frames. Both sides
assert the same cases, in `tests/timer.rs` and `web/shared.test.mjs`. Change one
and change the other, or the stage display and the server will disagree about
what the timer says.

**Saved state stays loadable.** Every field of `RoomState` carries a serde
default, so a snapshot from an older build still loads. Adding a field without
one breaks every existing snapshot, and `load_all` skips a file it cannot read,
so the failure is silent. `tests/persist.rs` holds a snapshot written in an older
shape to catch exactly that.

## Testing

Tests are integration level and live in `tests/`, one file per area. They use the
public API of the library rather than reaching inside it, so a refactor that keeps
behavior keeps the tests.

- The timer, the rundown and the commands test at fixed timestamps. Nothing
  sleeps
- Route and API tests run a real axum app
- Socket tests run a real server on an ephemeral port and connect over WebSocket
- `tests/discovery_live.rs` needs working multicast on the machine
- `tests/drift.rs` takes ten seconds and stays out of the default run. `make soak`
  runs it and prints the numbers

The JavaScript suites cover the parts with real logic: the readout, the offset
estimator, duration and clock parsing, the agenda projection, the schedule
totals and the per-screen overrides. A browser verifies the rendering, which no
assertion covers.

## Verifying the frontend

There is no headless browser here, so drive a real one against a running server.
Two habits are worth keeping:

- Change room state over the HTTP API rather than by typing into the console.
  It is faster, and it is the same path a controller uses
- Check rendering on a foreground window. A backgrounded tab throttles timers
  and animation frames, which looks exactly like a bug that is not there

## Style

`make lint` is the gate: `clippy` with warnings denied, and `cargo fmt --check`.

Comments explain what the code cannot. A constraint, a trap for the next reader,
or a rule that looks wrong until you know why. Rationale and history belong in
the commit message, where `git log` and `git blame` can find them.
