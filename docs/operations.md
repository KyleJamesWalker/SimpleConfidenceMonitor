# Running a session

Everything an operator needs during an event: the screens, the flags, the API,
and what to do when something goes wrong.

## The screens

Give a room a name. The server creates it on first request, so there is nothing
to configure.

| Screen | URL | Needs the token |
|---|---|---|
| Stage display | `http://<host>:8080/keynote` | no |
| Operator console | `http://<host>:8080/keynote/edit` | yes |
| Agenda | `http://<host>:8080/keynote/agenda` | no |
| Room picker | `http://<host>:8080/` | no |

The picker builds both links for a room and shows a QR code for the viewer, so a
phone reaches the stage display without anyone typing an address.

A room name is 1 to 64 characters of `a-z`, `0-9`, dash and underscore, and the
server lowercases what it gets. `api`, `assets` and `healthz` are reserved.

An API read does not bring a room into being, and reading a room that does not
exist answers with the defaults. A command creates one, and so does a connected
socket: opening a screen therefore creates the room a moment later, when its
socket connects. A typo in a `curl` leaves nothing behind, a typo in a browser
leaves an empty room.

## Flags

| Flag | Default | Controls |
|---|---|---|
| `--port` | `8080` | Listening port |
| `--bind` | `0.0.0.0` | Listening address. The default lets a stage display reach the server |
| `--token` | none | Token required to open the console and to send commands. Also read from `SCM_TOKEN` |
| `--state-dir` | none | Directory for room snapshots. Without it, state stays in memory |
| `--mdns` | off | Advertise the server on the local network |
| `--name` | the port | Name to advertise, when `--mdns` is on |

Started without `--token`, the server logs a warning and leaves the console
open. Viewer routes never ask for the token, because a display machine in a
booth cannot type one.

A browser that opens the console without the token gets a form asking for it,
rather than a refusal. Entering it once stores an `HttpOnly` cookie that lasts a
day, so every room on that server opens without asking again. A link carrying
`?token=<value>` skips the form, which is what the picker builds when its token
field is filled.

Scripts send `Authorization: Bearer <value>` and get a plain refusal, since a
form is no use to `curl`.

## During the show

The console carries the timer, the message to the speaker, the screen controls,
the second timer and the rundown. Keys cover the hot path.

| Key | Action |
|---|---|
| `space` | Start or pause |
| `r` | Reset |
| `b` | Blackout |
| `f` | Flash |
| `n` and `p` | Next and previous cue |
| `enter` | Send the message, from the message box |

A row of quick messages sits above the message box, so the common notes take one
press. Edit them on the console, or replace them with `set_presets`. A room holds
at most eight.

The chime sounds a tone when the timer reaches zero. It starts off. A browser
blocks sound until someone interacts with the page, so a viewer with the chime on
shows a button to tap once. Only the crossing into overtime rings, so a screen
that joins late stays silent.

## Per-screen overrides

A query parameter changes one screen without touching the room, so a booth
monitor can differ from the stage feed:

```
/keynote?clock=0&progress=0&mirror=1&scale=140&title=Booth
```

`clock`, `progress`, `mirror`, `blackout`, `sound`, `aux`, `speaker` and `next`
and `notes` each take `1` or `0`. `scale` takes a percent and clamps to 50 and
200. `title` replaces the title text, and an empty `title=` blanks the line.

Use `mirror` for a display behind teleprompter glass.

## The API

Every operator action is an HTTP call, so Bitfocus Companion, a Stream Deck or a
shell script can drive the show.

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
| `message` | `text`, `tone`, `visible` | Note to the speaker. Every field is optional |
| `send_preset` | `index` | Sends the quick message at that position |
| `set_presets` | `presets` | Replaces the quick messages. Each carries `text` and `tone` |
| `flash` | | Flashes the viewer twice |
| `blackout` | `on` | Cuts the viewer to black |
| `display` | `title`, `next_up`, `show_clock`, `clock_24h`, `show_progress`, `mirror`, `scale`, `chime`, `show_speaker`, `show_notes` | Screen settings. Every field is optional |
| `add_cue` | `title`, `speaker`, `duration_ms`, `notes` | Appends a cue |
| `update_cue` | `id`, and any cue field | Changes one cue |
| `remove_cue` | `id` | Drops a cue |
| `move_cue` | `id`, `to` | Reorders |
| `load_cue` | `id` | Points the timer and the screen at a cue |
| `next_cue`, `prev_cue` | | Walks the list |
| `set_auto_advance` | `on` | Starts the next cue when one runs out |
| `set_cues` | `cues` | Replaces the running order |
| `aux_start`, `aux_pause`, `aux_reset` | | Transport for the second timer |
| `aux_set_duration` | `ms` | Sets the second timer |
| `aux_adjust` | `ms` | Adds or removes time on the second timer |
| `aux_set` | `label`, `visible` | Names the second timer and shows or hides it |
| `clear_room` | | Returns every part of the room to its defaults |

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
| `POST` | `/api/rooms/<room>/rundown` | Replaces the running order from CSV or JSON |
| `GET` | `/api/qr?text=<url>` | An SVG QR code |
| `GET` | `/healthz` | `ok` |

## A running order from a spreadsheet

Export the CSV, edit it, and post it back:

```bash
curl http://localhost:8080/api/rooms/keynote/rundown.csv -o rundown.csv
curl -X POST http://localhost:8080/api/rooms/keynote/rundown \
  -H 'content-type: text/csv' --data-binary @rundown.csv
```

The columns are `title`, `speaker`, `duration` and `notes`. A header row names
them in any order, and common spellings map onto the same column, so `cue`,
`presenter` and `length` work too. A duration takes minutes, `mm:ss` or
`hh:mm:ss`, and an empty one falls back to five minutes.

A row without a title is an error, and a refused import leaves the running order
alone. The console carries the same import and export beside the cue list.

## Starting on the clock

A cue can wait for the clock instead of a press. Arm a time on the console, or
post the epoch millisecond:

```bash
curl -X POST http://localhost:8080/api/rooms/keynote/cmd \
  -H 'content-type: application/json' \
  -d '{"cmd": "schedule_start", "at_ms": 1767225600000}'
```

The timer waits at the top and the viewer counts down to the start. Starting by
hand, resetting, or loading a cue all cancel a pending start. A time already past
starts within a fifth of a second.

## State across a restart

With `--state-dir`, the server writes each room to `<dir>/<room>.json` one second
after it settles. Rooms load again at startup. A timer that was running comes
back paused at its saved elapsed time, because a restart means the show already
stopped.

Deleting a room deletes its snapshot. Clearing a room leaves the file in place
and overwrites it with the defaults.

A delete also closes the sockets on that room. A console watching it drops and
reconnects to an empty room, rather than carrying on against a room nobody else
can reach.

## Finding the server

With `--mdns`, the server advertises itself as `_scm._tcp.local.`, so a phone on
the same network can find it without anyone reading an IP address aloud.

```bash
dns-sd -B _scm._tcp local        # macOS
avahi-browse -r _scm._tcp        # Linux
```

It stays off by default, because a process that multicasts unasked surprises
people and trips endpoint security on a managed laptop. A network that blocks
multicast logs a warning and keeps serving.

## When something goes wrong

**The stage display shows a reconnecting badge.** It lost the socket and is
counting from its last anchor. The timer on screen is still close to right. Check
that the laptop has not changed network or gone to sleep.

**The console says zero viewers.** The display is on a different room name.
Compare the two URLs, or reissue both from the picker.

**The digits look frozen.** A browser throttles a hidden tab. Bring the window
forward, or give the display machine its own window in the foreground.

**The chime never sounds.** A browser blocks audio until the page is clicked. The
viewer shows a tap-to-enable button whenever the chime is on and sound is still
locked.

**The display sleeps mid-session.** The viewer holds a screen wake lock where the
browser supports it, and offers a fullscreen button. Turn off system sleep on the
display machine as well.

**A snapshot did not come back.** Unreadable files and names that are not valid
rooms are skipped at startup rather than failing the boot. Check the log line
that reports how many rooms were restored.
