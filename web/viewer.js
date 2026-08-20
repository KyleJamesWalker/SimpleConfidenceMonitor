import {
  activeCue,
  RoomSocket,
  formatClock,
  formatDuration,
  readout,
  roomFromPath,
  shouldChime,
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
  soundHint: document.getElementById('soundHint'),
  armed: document.getElementById('armed'),
  speaker: document.getElementById('speaker'),
  notes: document.getElementById('notes'),
  aux: document.getElementById('aux'),
  auxLabel: document.getElementById('auxLabel'),
  auxTime: document.getElementById('auxTime'),
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
  const cue = activeCue(frame.rundown);
  setText(el.speaker, pick('speaker', display?.show_speaker) ? cue?.speaker : '');
  setText(el.notes, pick('notes', display?.show_notes) ? cue?.notes : '');
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
    if (chimeWanted() && shouldChime(painted.phase, phase)) playChime();
    el.timer.className = `timer ${phase}${phase === 'expired' ? ' blink' : ''}`;
    el.bar.className = `bar ${phase}`;
    painted.phase = phase;
  }
  const askForSound = chimeWanted() && !audio;
  if (el.soundHint.hidden === askForSound) el.soundHint.hidden = !askForSound;

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

  const aux = state.aux;
  const showAux = pick('aux', aux?.visible);
  if (painted.showAux !== showAux) {
    el.aux.hidden = !showAux;
    painted.showAux = showAux;
  }
  if (showAux) {
    const auxOut = readout(aux.timer, now);
    const auxText = formatDuration(auxOut.valueMs);
    if (painted.auxTime !== auxText) {
      el.auxTime.textContent = auxText;
      el.auxTime.className = `auxTime ${auxOut.phase}`;
      painted.auxTime = auxText;
    }
    if (painted.auxLabel !== aux.label) {
      el.auxLabel.textContent = aux.label;
      painted.auxLabel = aux.label;
    }
  }

  const armedAt = state.timer.start_at_ms;
  const armedText = armedAt && !out.running ? `starts in ${formatDuration(armedAt - now)}` : '';
  if (painted.armed !== armedText) {
    el.armed.textContent = armedText;
    el.armed.hidden = !armedText;
    painted.armed = armedText;
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

function chimeWanted() {
  return pick('sound', state?.display?.chime);
}

let audio = null;

// Autoplay policy blocks sound until a gesture, so the context waits for one.
function unlockAudio() {
  if (audio) return;
  const Context = window.AudioContext || window.webkitAudioContext;
  if (!Context) return;
  audio = new Context();
  audio.resume?.();
}

function playChime() {
  if (!audio) return;
  audio.resume?.();
  const start = audio.currentTime;
  for (let beep = 0; beep < 3; beep += 1) {
    const at = start + beep * 0.22;
    const tone = audio.createOscillator();
    const gain = audio.createGain();
    tone.type = 'sine';
    tone.frequency.value = 880;
    // Ramp both ends, or the speaker clicks.
    gain.gain.setValueAtTime(0, at);
    gain.gain.linearRampToValueAtTime(0.28, at + 0.01);
    gain.gain.linearRampToValueAtTime(0, at + 0.16);
    tone.connect(gain).connect(audio.destination);
    tone.start(at);
    tone.stop(at + 0.18);
  }
}

for (const event of ['click', 'keydown', 'touchstart']) {
  document.addEventListener(event, unlockAudio, { once: true, passive: true });
}

el.soundHint.addEventListener('click', () => {
  unlockAudio();
  el.soundHint.hidden = true;
});

document.addEventListener('click', requestWakeLock, { once: true });
async function requestWakeLock() {
  try {
    await navigator.wakeLock?.request('screen');
  } catch {
    // A refusal is not fatal, so there is nothing to handle.
  }
}
