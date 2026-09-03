// Shared socket client and timer math. The viewer and the console both use this.

export const MIN = 60000;

// WebSocket.OPEN, spelled out so this module also runs under a test double.
const OPEN = 1;

// How long the socket waits before its first reconnect, and the ceiling it
// doubles up to.
export const INITIAL_BACKOFF_MS = 250;
export const MAX_BACKOFF_MS = 5000;

// Capped doubling, so a server that stays down is polled at a steady rate.
export function nextBackoffMs(current) {
  return Math.min(current * 2, MAX_BACKOFF_MS);
}

// Keydowns the focused control owns. A text field takes every key. A button or
// a link takes the two that activate it, but only when the operator tabbed to
// it: a click leaves a button focused, and Space is the transport's key.
export function targetOwnsKey(tagName, key, keyboardFocused = false) {
  if (['INPUT', 'TEXTAREA', 'SELECT'].includes(tagName)) return true;
  if (tagName !== 'BUTTON' && tagName !== 'A') return false;
  return keyboardFocused && (key === ' ' || key === 'Enter');
}

async function askAuth() {
  try {
    const response = await fetch('/api/auth', { cache: 'no-store' });
    return response.status;
  } catch {
    // No answer at all is the network, not the token.
    return 0;
  }
}

// The server owns the clock. Each client estimates its offset so a running
// timer renders smoothly between state frames.
export class RoomSocket {
  constructor({
    room,
    role,
    onState,
    onStatus,
    onError,
    onRefused,
    openSocket,
    probeAuth,
    schedule,
  }) {
    this.room = room;
    this.role = role;
    this.onState = onState || (() => {});
    this.onStatus = onStatus || (() => {});
    this.onError = onError || (() => {});
    this.onRefused = onRefused || (() => {});
    this.openSocket = openSocket || ((url) => new WebSocket(url));
    this.probeAuth = probeAuth || askAuth;
    this.schedule = schedule || ((run, wait) => setTimeout(run, wait));
    this.offsets = [];
    this.offsetMs = 0;
    this.backoffMs = INITIAL_BACKOFF_MS;
    this.socket = null;
    this.handshook = false;
    this.connect();
    if (typeof document !== 'undefined') {
      document.addEventListener('visibilitychange', () => {
        if (!document.hidden && this.socket?.readyState !== OPEN) this.connect();
      });
    }
  }

  connect() {
    if (this.socket && this.socket.readyState <= OPEN) return;
    const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
    const url = `${scheme}://${location.host}/api/rooms/${this.room}/ws?role=${this.role}`;
    this.onStatus('connecting');
    this.handshook = false;
    const socket = this.openSocket(url);
    this.socket = socket;

    socket.addEventListener('open', () => {
      this.handshook = true;
      this.backoffMs = INITIAL_BACKOFF_MS;
      this.onStatus('online');
      this.probeClock();
    });
    socket.addEventListener('message', (event) => this.receive(event.data));
    socket.addEventListener('close', () => this.retry());
    socket.addEventListener('error', () => this.retry());
  }

  async retry() {
    // An edit socket rides a cookie that expires. A handshake the server
    // refused is not a network drop, and reconnecting on backoff would leave
    // the operator staring at `offline` with no way to enter the token again.
    if (this.role === 'edit' && !this.handshook && (await this.probeAuth()) === 401) {
      this.onStatus('unauthorized');
      this.onRefused();
      return;
    }
    this.onStatus('offline');
    const wait = this.backoffMs;
    this.backoffMs = nextBackoffMs(this.backoffMs);
    clearTimeout(this.retryTimer);
    this.retryTimer = this.schedule(() => this.connect(), wait);
  }

  receive(raw) {
    let frame;
    try {
      frame = JSON.parse(raw);
    } catch {
      return;
    }
    if (frame.type === 'state') {
      this.state = frame;
      this.onState(frame);
    } else if (frame.type === 'pong') {
      this.recordOffset(frame);
    } else if (frame.type === 'error') {
      this.onError(frame.message);
    }
  }

  probeClock() {
    this.offsets = [];
    for (let i = 0; i < 3; i += 1) {
      setTimeout(() => this.send({ cmd: 'ping', client_time_ms: Date.now() }), i * 60);
    }
  }

  recordOffset(frame) {
    this.offsets.push({
      sentMs: frame.client_time_ms,
      receivedMs: Date.now(),
      serverMs: frame.server_time_ms,
    });
    this.offsetMs = medianOffset(this.offsets);
  }

  serverNow() {
    return Date.now() + this.offsetMs;
  }

  send(message) {
    if (this.socket?.readyState === OPEN) {
      this.socket.send(JSON.stringify(message));
    }
  }
}

// One estimate of how far the server clock sits ahead of this one. Half the
// round trip is charged to each direction, which is the best a single sample
// can do.
export function offsetSample({ sentMs, receivedMs, serverMs }) {
  const rtt = receivedMs - sentMs;
  return serverMs + rtt / 2 - receivedMs;
}

// The median, so one stalled response cannot drag the clock.
export function medianOffset(samples) {
  if (!samples.length) return 0;
  const sorted = samples.map(offsetSample).sort((a, b) => a - b);
  return sorted[Math.floor((sorted.length - 1) / 2)];
}

export function elapsedMs(timer, serverNow) {
  if (timer.run.state !== 'running') return timer.elapsed_ms;
  return timer.elapsed_ms + Math.max(0, serverNow - timer.run.since_ms);
}

// Mirrors Timer::readout in src/timer.rs. Both must agree.
export function readout(timer, serverNow) {
  const elapsed = elapsedMs(timer, serverNow);
  const remaining = timer.duration_ms - elapsed;
  let value;
  if (timer.mode === 'countdown') {
    value = timer.on_expire === 'hold_at_zero' ? Math.max(0, remaining) : remaining;
  } else {
    value = elapsed;
  }
  return {
    valueMs: value,
    elapsedMs: elapsed,
    remainingMs: remaining,
    phase: phaseOf(timer, remaining),
    progress: timer.duration_ms ? Math.min(1, Math.max(0, elapsed / timer.duration_ms)) : 0,
    running: timer.run.state === 'running',
  };
}

function phaseOf(timer, remaining) {
  if (timer.mode === 'time_of_day' || timer.duration_ms === 0) return 'normal';
  if (remaining <= 0) return 'expired';
  if (timer.danger_ms > 0 && remaining <= timer.danger_ms) return 'danger';
  if (timer.warn_ms > 0 && remaining <= timer.warn_ms) return 'warn';
  return 'normal';
}

// 12:34 under an hour, 1:02:03 above it, minus sign in overtime.
export function formatDuration(ms) {
  const sign = ms < 0 ? '-' : '';
  const total = Math.floor(Math.abs(ms) / 1000);
  const hours = Math.floor(total / 3600);
  const minutes = Math.floor((total % 3600) / 60);
  const seconds = total % 60;
  const pad = (n) => String(n).padStart(2, '0');
  return hours > 0
    ? `${sign}${hours}:${pad(minutes)}:${pad(seconds)}`
    : `${sign}${minutes}:${pad(seconds)}`;
}

// The same clock without the seconds, for a table that shows minutes. Leaving
// the seconds in would freeze them: the agenda repaints on the minute.
export function formatClockToMinute(date, use24h) {
  const text = formatClock(date, use24h);
  return use24h ? text.slice(0, 5) : text.replace(/:\d{2}( [AP]M)$/, '$1');
}

export function formatClock(date, use24h) {
  const hours = date.getHours();
  const shown = use24h ? hours : hours % 12 || 12;
  const suffix = use24h ? '' : hours < 12 ? ' AM' : ' PM';
  return `${use24h ? String(shown).padStart(2, '0') : shown}:${String(date.getMinutes()).padStart(2, '0')}:${String(date.getSeconds()).padStart(2, '0')}${suffix}`;
}

// Accepts minutes, mm:ss, or hh:mm:ss. Returns null when the text is not a duration.
export function parseDuration(raw) {
  const text = String(raw).trim();
  if (!text) return null;
  const parts = text.split(':');
  if (parts.length > 3 || parts.some((part) => !/^\d+$/.test(part))) return null;
  if (parts.length === 1) return Math.round(Number(parts[0]) * MIN);
  return parts.map(Number).reduce((total, part) => total * 60 + part, 0) * 1000;
}

// The next occurrence of a wall clock time. Today when it is still ahead,
// tomorrow otherwise, so an operator never arms a start that already passed.
export function nextClockTime(raw, nowMs) {
  const parts = String(raw).trim().split(':');
  if (parts.length > 3 || parts.some((part) => !/^\d{1,2}$/.test(part))) return null;
  const [hours, minutes = 0, seconds = 0] = parts.map(Number);
  if (hours > 23 || minutes > 59 || seconds > 59) return null;
  const now = new Date(nowMs);
  const at = new Date(now.getFullYear(), now.getMonth(), now.getDate(), hours, minutes, seconds, 0);
  if (at.getTime() <= nowMs) at.setDate(at.getDate() + 1);
  return at.getTime();
}

// One screen can differ from the room without touching it. A flag reads true
// unless it is zero. scale takes a number, and title replaces the text.
export function screenOverrides(search) {
  const params = new URLSearchParams(search);
  const flag = (key) => (params.has(key) ? params.get(key) !== '0' : null);
  const overrides = {
    clock: flag('clock'),
    progress: flag('progress'),
    mirror: flag('mirror'),
    blackout: flag('blackout'),
    sound: flag('sound'),
    aux: flag('aux'),
    speaker: flag('speaker'),
    notes: flag('notes'),
    next: flag('next'),
    scale: null,
    title: params.has('title') ? params.get('title') : null,
  };
  if (params.has('scale')) {
    const asked = Number(params.get('scale'));
    const given = params.get('scale').trim();
    if (given !== '' && Number.isFinite(asked)) {
      overrides.scale = Math.min(200, Math.max(50, Math.round(asked)));
    }
  }
  return overrides;
}

// What the agenda repaints on. Every field the table shows belongs here, or an
// edit to it leaves the page stale until something else moves. Times round to
// the minute the table prints: before a show every projected time tracks the
// wall clock, and raw milliseconds would rebuild every row twice a second.
export function agendaSignature(rows) {
  return JSON.stringify(
    rows.map((row) => [
      row.id,
      row.state,
      minuteOf(row.startMs),
      minuteOf(row.endMs),
      row.title,
      row.speaker,
      row.durationMs,
    ]),
  );
}

function minuteOf(ms) {
  return ms === null || ms === undefined ? null : Math.floor(ms / MIN);
}

export function activeCue(rundown) {
  const cues = rundown?.cues || [];
  return cues.find((cue) => cue.id === rundown.active) || null;
}

// Clock times for a rundown. The active cue anchors on the wall clock, and the
// rest chain from it. An overrunning cue chains the rest from now, because a
// cue cannot start in the past.
export function projectAgenda(rundown, activeRemainingMs, nowMs) {
  const cues = rundown.cues || [];
  const activeIndex = cues.findIndex((cue) => cue.id === rundown.active);
  let chainFrom = nowMs;
  if (activeIndex >= 0) {
    chainFrom = Math.max(nowMs, nowMs + activeRemainingMs);
  }
  return cues.map((cue, index) => {
    const row = {
      id: cue.id,
      index,
      title: cue.title,
      speaker: cue.speaker,
      durationMs: cue.duration_ms,
      startMs: null,
      endMs: null,
      state: 'planned',
    };
    if (activeIndex >= 0 && index < activeIndex) {
      row.state = 'done';
      return row;
    }
    if (index === activeIndex) {
      row.state = 'active';
      row.startMs = nowMs - (cue.duration_ms - activeRemainingMs);
      row.endMs = nowMs + activeRemainingMs;
      return row;
    }
    row.startMs = chainFrom;
    row.endMs = chainFrom + cue.duration_ms;
    chainFrom = row.endMs;
    return row;
  });
}

// Only the crossing into overtime rings. A viewer joining a room that is
// already over does not, because prev is null on its first frame.
export function shouldChime(previousPhase, nextPhase) {
  return Boolean(previousPhase) && previousPhase !== 'expired' && nextPhase === 'expired';
}

// Time left in the plan: what remains of the active cue, plus every cue after it.
export function rundownTotals(rundown, activeRemainingMs) {
  const cues = rundown.cues || [];
  const totalMs = cues.reduce((sum, cue) => sum + cue.duration_ms, 0);
  const activeIndex = cues.findIndex((cue) => cue.id === rundown.active);
  const afterMs = cues
    .slice(activeIndex + 1)
    .reduce((sum, cue) => (activeIndex < 0 ? sum : sum + cue.duration_ms), 0);
  const remainingMs =
    activeIndex < 0 ? totalMs : Math.max(0, activeRemainingMs) + afterMs;
  return {
    cueCount: cues.length,
    activeIndex,
    totalMs,
    remainingMs,
    doneMs: Math.max(0, totalMs - remainingMs),
  };
}

// The server rejects anything outside a-z, 0-9, dash and underscore.
export function normalizeRoomName(raw) {
  return String(raw)
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9-_]+/g, '-')
    .replace(/^-+|-+$/g, '')
    .slice(0, 64);
}

export function roomLinks(origin, room, token) {
  const suffix = token ? `?token=${encodeURIComponent(token)}` : '';
  return {
    viewer: `${origin}/${room}`,
    console: `${origin}/${room}/edit${suffix}`,
    agenda: `${origin}/${room}/agenda`,
  };
}

export function roomFromPath() {
  return location.pathname.split('/').filter(Boolean)[0] || 'main';
}
