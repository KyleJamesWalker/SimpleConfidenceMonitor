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

  // Median of three samples, each corrected for half the round trip.
  recordOffset(frame) {
    const rtt = Date.now() - frame.client_time_ms;
    this.offsets.push(frame.server_time_ms + rtt / 2 - Date.now());
    const sorted = [...this.offsets].sort((a, b) => a - b);
    this.offsetMs = sorted[Math.floor(sorted.length / 2)];
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

export function roomFromPath() {
  return location.pathname.split('/').filter(Boolean)[0] || 'main';
}
