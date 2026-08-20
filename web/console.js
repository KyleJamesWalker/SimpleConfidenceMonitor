import { MIN, RoomSocket, formatDuration, readout, roomFromPath } from '/assets/shared.js';

const el = (id) => document.getElementById(id);
const room = roomFromPath();
document.title = `${room} — console`;
el('roomName').textContent = room;
el('viewerLink').href = `/${room}`;

const QUICK_MINUTES = [5, 10, 15, 20, 30];

let state = null;
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

function applyState(frame) {
  el('clients').textContent = `${frame.viewers} viewer${frame.viewers === 1 ? '' : 's'}`;
  if (document.activeElement !== el('mode')) el('mode').value = frame.timer.mode;
  el('runLabel').textContent = frame.timer.run.state;
  el('modeLabel').textContent = frame.timer.mode.replace(/_/g, ' ');
  el('start').textContent = frame.timer.run.state === 'running' ? 'Running' : 'Start';
  render();
}

function render() {
  if (!state) return;
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
  requestAnimationFrame(render);
}
requestAnimationFrame(render);

el('start').addEventListener('click', () => send({ cmd: 'start' }));
el('pause').addEventListener('click', () => send({ cmd: 'pause' }));
el('reset').addEventListener('click', () => send({ cmd: 'reset' }));
el('mode').addEventListener('change', (event) => send({ cmd: 'set_mode', mode: event.target.value }));

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
});

export function parseDuration(raw) {
  const text = String(raw).trim();
  if (!text) return null;
  const parts = text.split(':');
  if (parts.length > 3 || parts.some((part) => part !== '' && !/^\d+$/.test(part))) return null;
  if (parts.length === 1) return Math.round(Number(parts[0]) * MIN);
  const seconds = parts
    .map((part) => Number(part || 0))
    .reduce((total, part) => total * 60 + part, 0);
  return seconds * 1000;
}

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
