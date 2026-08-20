import {
  MIN,
  RoomSocket,
  formatDuration,
  parseDuration,
  readout,
  roomFromPath,
} from '/assets/shared.js';

const el = (id) => document.getElementById(id);
const room = roomFromPath();
document.title = `${room} — console`;
el('roomName').textContent = room;
el('viewerLink').href = `/${room}`;

const QUICK_MINUTES = [5, 10, 15, 20, 30];
const TOGGLES = {
  blackout: (on) => ({ cmd: 'blackout', on }),
  showClock: (on) => ({ cmd: 'display', show_clock: on }),
  clock24h: (on) => ({ cmd: 'display', clock_24h: on }),
  showProgress: (on) => ({ cmd: 'display', show_progress: on }),
  mirror: (on) => ({ cmd: 'display', mirror: on }),
};
const TOGGLE_STATE = {
  blackout: (frame) => frame.display.blackout,
  showClock: (frame) => frame.display.show_clock,
  clock24h: (frame) => frame.display.clock_24h,
  showProgress: (frame) => frame.display.show_progress,
  mirror: (frame) => frame.display.mirror,
};

let state = null;
let tone = 'neutral';
const painted = {};

const socket = new RoomSocket({
  room,
  role: 'edit',
  onState: (frame) => {
    state = frame;
    applyState(frame);
  },
  onStatus: (status) => {
    const node = el('status');
    node.textContent = status;
    node.className = `status ${status}`;
  },
  onError: toast,
});

const send = (message) => socket.send(message);
const editing = (id) => document.activeElement === el(id);

// A field the operator has edited is a draft. A state frame must not clobber it.
const drafts = new Set();

function syncField(id, serverValue) {
  const node = el(id);
  if (editing(id) || drafts.has(id)) {
    if (node.value === String(serverValue)) drafts.delete(id);
    return;
  }
  node.value = serverValue;
}

for (const id of ['title', 'nextUp', 'message', 'warn', 'danger', 'duration']) {
  el(id).addEventListener('input', () => drafts.add(id));
}

function applyState(frame) {
  const { timer, display, message } = frame;
  el('clients').textContent = `${frame.viewers} viewer${frame.viewers === 1 ? '' : 's'}`;
  el('runLabel').textContent = timer.run.state;
  el('modeLabel').textContent = timer.mode.replace(/_/g, ' ');
  el('start').textContent = timer.run.state === 'running' ? 'Running' : 'Start';
  if (!editing('mode')) el('mode').value = timer.mode;
  if (!editing('onExpire')) el('onExpire').value = timer.on_expire;
  syncField('warn', formatDuration(timer.warn_ms));
  syncField('danger', formatDuration(timer.danger_ms));
  syncField('title', display.title);
  syncField('nextUp', display.next_up);
  syncField('message', message.text);
  if (!editing('scale')) {
    el('scale').value = display.scale;
    el('scaleOut').textContent = `${display.scale}%`;
  }

  tone = message.tone;
  for (const button of document.querySelectorAll('.tone')) {
    button.classList.toggle('on', button.dataset.tone === tone);
  }
  for (const [id, read] of Object.entries(TOGGLE_STATE)) {
    el(id).classList.toggle('on', Boolean(read(frame)));
  }
  el('showMessage').classList.toggle('on', message.visible && Boolean(message.text));
  render();
}

function render() {
  if (state) {
    const out = readout(state.timer, socket.serverNow());
    const text = formatDuration(out.valueMs);
    if (painted.timer !== text) {
      el('timer').textContent = text;
      painted.timer = text;
    }
    if (painted.phase !== out.phase) {
      el('timer').className = `timer ${out.phase}`;
      painted.phase = out.phase;
    }
  }
  requestAnimationFrame(render);
}
requestAnimationFrame(render);

el('start').addEventListener('click', () => send({ cmd: 'start' }));
el('pause').addEventListener('click', () => send({ cmd: 'pause' }));
el('reset').addEventListener('click', () => send({ cmd: 'reset' }));
el('flash').addEventListener('click', () => send({ cmd: 'flash' }));
el('mode').addEventListener('change', (e) => send({ cmd: 'set_mode', mode: e.target.value }));
el('onExpire').addEventListener('change', (e) =>
  send({ cmd: 'set_on_expire', on_expire: e.target.value }),
);

for (const button of document.querySelectorAll('[data-adjust]')) {
  button.addEventListener('click', () =>
    send({ cmd: 'adjust', ms: Number(button.dataset.adjust) }),
  );
}

const chips = el('chips');
for (const minutes of QUICK_MINUTES) {
  const button = document.createElement('button');
  button.textContent = `${minutes}m`;
  button.addEventListener('click', () => send({ cmd: 'set_duration', ms: minutes * MIN }));
  chips.append(button);
}

el('duration').addEventListener('change', (event) => {
  const ms = parseDuration(event.target.value);
  if (ms === null) {
    toast('Enter minutes, or mm:ss');
    return;
  }
  send({ cmd: 'set_duration', ms });
  event.target.value = '';
  drafts.delete('duration');
  event.target.blur();
});

for (const id of ['warn', 'danger']) {
  el(id).addEventListener('change', () => {
    const warn = parseDuration(el('warn').value);
    const danger = parseDuration(el('danger').value);
    if (warn === null || danger === null) {
      toast('Thresholds take minutes, or mm:ss');
      return;
    }
    send({ cmd: 'set_thresholds', warn_ms: warn, danger_ms: danger });
    el(id).blur();
  });
}

for (const [id, command] of Object.entries(TOGGLES)) {
  el(id).addEventListener('click', () => {
    if (!state) return;
    send(command(!TOGGLE_STATE[id](state)));
  });
}

el('scale').addEventListener('input', (event) => {
  el('scaleOut').textContent = `${event.target.value}%`;
  send({ cmd: 'display', scale: Number(event.target.value) });
});

for (const id of ['title', 'nextUp']) {
  const field = id === 'title' ? 'title' : 'next_up';
  el(id).addEventListener('input', (event) => {
    const value = event.target.value;
    clearTimeout(el(id).timer);
    el(id).timer = setTimeout(() => send({ cmd: 'display', [field]: value }), 250);
  });
}

for (const button of document.querySelectorAll('.tone')) {
  button.addEventListener('click', () => {
    tone = button.dataset.tone;
    send({ cmd: 'message', tone });
  });
}

el('showMessage').addEventListener('click', showMessage);
el('hideMessage').addEventListener('click', () => send({ cmd: 'message', visible: false }));

function showMessage() {
  send({ cmd: 'message', text: el('message').value, tone, visible: true });
}

el('message').addEventListener('keydown', (event) => {
  if (event.key === 'Enter' && !event.shiftKey) {
    event.preventDefault();
    showMessage();
  }
});

document.addEventListener('keydown', (event) => {
  const typing = ['INPUT', 'TEXTAREA', 'SELECT'].includes(event.target.tagName);
  if (typing || event.metaKey || event.ctrlKey || event.altKey) return;
  const running = state?.timer.run.state === 'running';
  const keys = {
    ' ': () => send({ cmd: running ? 'pause' : 'start' }),
    r: () => send({ cmd: 'reset' }),
    b: () => send({ cmd: 'blackout', on: !state?.display.blackout }),
    f: () => send({ cmd: 'flash' }),
  };
  const action = keys[event.key.toLowerCase()];
  if (action) {
    event.preventDefault();
    action();
  }
});

let toastTimer;
function toast(text) {
  const node = el('toast');
  node.textContent = text;
  node.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => {
    node.hidden = true;
  }, 3500);
}
