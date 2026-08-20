import {
  RoomSocket,
  formatClock,
  formatDuration,
  readout,
  roomFromPath,
} from '/assets/shared.js';

const el = {
  stage: document.getElementById('stage'),
  timer: document.getElementById('timer'),
  title: document.getElementById('title'),
  next: document.getElementById('next'),
  clock: document.getElementById('clock'),
  message: document.getElementById('message'),
  progress: document.getElementById('progress'),
  bar: document.getElementById('bar'),
  status: document.getElementById('status'),
  flash: document.getElementById('flash'),
  fullscreen: document.getElementById('fullscreen'),
};

const params = new URLSearchParams(location.search);
const override = (key) => (params.has(key) ? params.get(key) !== '0' : null);

const room = roomFromPath();
document.title = `${room} — confidence monitor`;

let state = null;
let lastFlash = 0;
let offlineSince = 0;
const painted = {};

const socket = new RoomSocket({
  room,
  role: 'view',
  onState: (frame) => {
    state = frame;
    applyState(frame);
  },
  onStatus: (status) => {
    if (status === 'online') {
      offlineSince = 0;
      el.status.hidden = true;
    } else if (!offlineSince) {
      offlineSince = Date.now();
    }
  },
});

function applyState(frame) {
  const { display, message, timer } = frame;
  if (display) {
    setText(el.title, display.show_title === false ? '' : display.title);
    setText(el.next, display.next_up ? `Next: ${display.next_up}` : '');
    el.stage.classList.toggle('mirror', pick('mirror', display.mirror));
    el.stage.classList.toggle('blackout-on', pick('blackout', display.blackout));
    el.progress.hidden = !pick('progress', display.show_progress);
    el.stage.style.setProperty('--scale', (display.scale || 100) / 100);
    if (display.flash_at && display.flash_at !== lastFlash) {
      lastFlash = display.flash_at;
      triggerFlash();
    }
  }
  if (message) {
    setText(el.message, message.text);
    el.message.className = `message ${message.tone || 'neutral'}${
      message.visible && message.text ? ' visible' : ''
    }`;
  }
  if (timer) render();
}

function pick(key, fallback) {
  const chosen = override(key);
  return chosen === null ? Boolean(fallback) : chosen;
}

function triggerFlash() {
  el.flash.classList.remove('on');
  void el.flash.offsetWidth;
  el.flash.classList.add('on');
}

function setText(node, text) {
  const value = text || '';
  if (node.textContent !== value) node.textContent = value;
}

function render() {
  if (!state) return;
  const now = socket.serverNow();
  const out = readout(state.timer, now);

  const text =
    state.timer.mode === 'time_of_day'
      ? formatClock(new Date(now), state.display?.clock_24h ?? true)
      : formatDuration(out.valueMs);
  if (painted.timer !== text) {
    el.timer.textContent = text;
    painted.timer = text;
  }

  const phase = out.phase;
  if (painted.phase !== phase) {
    el.timer.className = `timer ${phase}${phase === 'expired' ? ' blink' : ''}`;
    el.bar.className = `bar ${phase}`;
    painted.phase = phase;
  }

  const width = `${(out.progress * 100).toFixed(2)}%`;
  if (painted.width !== width) {
    el.bar.style.width = width;
    painted.width = width;
  }

  if (pick('clock', state.display?.show_clock ?? true)) {
    const clock = formatClock(new Date(now), state.display?.clock_24h ?? true);
    if (painted.clock !== clock) {
      el.clock.textContent = clock;
      painted.clock = clock;
    }
  } else if (painted.clock !== '') {
    el.clock.textContent = '';
    painted.clock = '';
  }

  const stale = offlineSince && Date.now() - offlineSince > 5000;
  if (el.status.hidden === Boolean(stale)) el.status.hidden = !stale;
}

function frame() {
  render();
  requestAnimationFrame(frame);
}
requestAnimationFrame(frame);

el.fullscreen.addEventListener('click', () => {
  if (document.fullscreenElement) document.exitFullscreen();
  else document.documentElement.requestFullscreen?.();
});

document.addEventListener('click', requestWakeLock, { once: true });
async function requestWakeLock() {
  try {
    await navigator.wakeLock?.request('screen');
  } catch {
    // A refusal is not fatal, so there is nothing to handle.
  }
}
