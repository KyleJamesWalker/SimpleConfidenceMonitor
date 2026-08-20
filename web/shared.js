// Shared socket client and timer math. The viewer and the console both use this.

export const MIN = 60000;

// The server owns the clock. Each client estimates its offset so a running
// timer renders smoothly between state frames.
export class RoomSocket {
  constructor({ room, role, onState, onStatus, onError }) {
    this.room = room;
    this.role = role;
    this.onState = onState || (() => {});
    this.onStatus = onStatus || (() => {});
    this.onError = onError || (() => {});
    this.offsets = [];
    this.offsetMs = 0;
    this.backoffMs = 250;
    this.socket = null;
    this.connect();
    document.addEventListener('visibilitychange', () => {
      if (!document.hidden && this.socket?.readyState !== WebSocket.OPEN) this.connect();
    });
  }

  connect() {
    if (this.socket && this.socket.readyState <= WebSocket.OPEN) return;
    const scheme = location.protocol === 'https:' ? 'wss' : 'ws';
    const url = `${scheme}://${location.host}/api/rooms/${this.room}/ws?role=${this.role}`;
    this.onStatus('connecting');
    const socket = new WebSocket(url);
    this.socket = socket;

    socket.addEventListener('open', () => {
      this.backoffMs = 250;
      this.onStatus('online');
      this.probeClock();
    });
    socket.addEventListener('message', (event) => this.receive(event.data));
    socket.addEventListener('close', () => this.retry());
    socket.addEventListener('error', () => this.retry());
  }

  retry() {
    this.onStatus('offline');
    const wait = this.backoffMs;
    this.backoffMs = Math.min(this.backoffMs * 2, 5000);
    clearTimeout(this.retryTimer);
    this.retryTimer = setTimeout(() => this.connect(), wait);
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
    if (this.socket?.readyState === WebSocket.OPEN) {
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
  };
}

export function roomFromPath() {
  return location.pathname.split('/').filter(Boolean)[0] || 'main';
}
