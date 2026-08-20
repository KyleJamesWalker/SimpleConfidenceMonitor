# Backlog

Work queued after the six spec milestones shipped. The top unchecked item is
next. Each item is one commit.

Ordering follows operator value: what a person running a session reaches for
most, first.

## Features

- [x] **Message presets.** Named quick messages on the console, one press to
      send. Every product in the survey has them, and typing during a talk is
      the thing an operator has no hands for.
- [x] **Sound on expiry.** A tone when a cue reaches zero, unlocked by the first
      click on the viewer. Browser autoplay policy blocks it until then.
- [x] **Agenda view.** A read-only `/<room>/agenda` page listing the rundown
      with planned clock times, for backstage and for the speakers.
- [x] **Rundown import and export.** CSV and JSON, both directions, so a
      running order can come from a spreadsheet.
- [x] **Scheduled start.** Start a cue at a wall clock time rather than on a
      press.
- [x] **Auxiliary timer.** A second independent countdown, for a break or a
      hard stop, shown alongside the main timer.
- [x] **GET command endpoint.** `GET /api/rooms/<room>/cmd?cmd=start` for
      hardware controllers that can only issue a GET.
- [x] **Clear a room.** An endpoint and a console control to reset a room to its
      defaults, including its snapshot.
- [x] **Speaker and notes on the viewer.** Optional lines, so a confidence
      monitor can carry the cue note.
- [x] **Discovery over mDNS.** Advertise the server so nobody reads an IP
      address aloud. Open question in the spec.

## Polish

- [x] **Toggle styling.** A toggle that is on looks the same as a primary
      action, because both are blue. The on state has to read at a glance.
- [ ] **Console layout.** The nudge row wraps badly in a narrow column, and the
      readout panel wastes vertical space.
- [x] **Reduced motion.** The overtime blink and the flash ignore
      `prefers-reduced-motion`.
- [ ] **More per-screen overrides.** `scale`, `title` and `next` are settable
      per room but not per screen.

- [ ] **Preset editing on the console.** `set_presets` changes the quick
 messages, but only over the API. The console shows them and cannot edit
 them.

## Verification

- [ ] **Drift soak test.** Measure the residual clock offset over a long run,
      which is the assumption the whole sync design rests on.
