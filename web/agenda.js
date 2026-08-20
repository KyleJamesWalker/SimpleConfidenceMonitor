import {
  RoomSocket,
  formatClock,
  formatDuration,
  projectAgenda,
  readout,
  roomFromPath,
  rundownTotals,
} from '/assets/shared.js';

const el = (id) => document.getElementById(id);
const room = roomFromPath();
document.title = `${room} — agenda`;
el('roomName').textContent = room;

let state = null;
const painted = {};

const socket = new RoomSocket({
  room,
  role: 'view',
  onState: (frame) => {
    state = frame;
    draw();
  },
  onStatus: (status) => {
    el('status').textContent = status;
    el('status').className = `status ${status}`;
  },
});

function draw() {
  if (!state) return;
  const now = socket.serverNow();
  const out = readout(state.timer, now);
  const rows = projectAgenda(state.rundown, out.remainingMs, now);
  const signature = JSON.stringify(rows.map((row) => [row.id, row.state, row.startMs, row.title]));
  if (painted.signature === signature) return;
  painted.signature = signature;

  const body = el('rows');
  body.replaceChildren();
  for (const row of rows) {
    body.append(rowNode(row));
  }
  el('empty').hidden = rows.length > 0;

  const active = rows.find((row) => row.state === 'active');
  el('current').hidden = !active;
  if (active) {
    el('currentTitle').textContent = active.title || '(untitled)';
    el('currentSpeaker').textContent = active.speaker;
  }
}

function rowNode(row) {
  const item = document.createElement('tr');
  item.className = row.state;

  const index = document.createElement('td');
  index.className = 'num';
  index.textContent = row.index + 1;

  const title = document.createElement('td');
  title.className = 'title';
  title.textContent = row.title || '(untitled)';
  if (row.speaker) {
    const who = document.createElement('span');
    who.className = 'who';
    who.textContent = ` ${row.speaker}`;
    title.append(who);
  }

  const start = document.createElement('td');
  start.className = 'time';
  start.textContent = row.startMs === null ? '' : clockOf(row.startMs);

  const end = document.createElement('td');
  end.className = 'time';
  end.textContent = row.endMs === null ? '' : clockOf(row.endMs);

  const length = document.createElement('td');
  length.className = 'time';
  length.textContent = formatDuration(row.durationMs);

  item.append(index, title, start, end, length);
  return item;
}

function clockOf(ms) {
  const at = new Date(ms);
  const use24h = state?.display?.clock_24h ?? true;
  return formatClock(at, use24h).slice(0, use24h ? 5 : undefined);
}

function tick() {
  if (state) {
    const now = socket.serverNow();
    const out = readout(state.timer, now);
    const timer = formatDuration(out.valueMs);
    if (painted.timer !== timer) {
      el('timer').textContent = timer;
      el('timer').className = `timer ${out.phase}`;
      painted.timer = timer;
    }
    const clock = formatClock(new Date(now), state.display?.clock_24h ?? true);
    if (painted.clock !== clock) {
      el('clock').textContent = clock;
      painted.clock = clock;
    }
    const totals = rundownTotals(state.rundown, out.remainingMs);
    const left = totals.cueCount ? `${formatDuration(totals.remainingMs)} left` : '';
    if (painted.left !== left) {
      el('left').textContent = left;
      painted.left = left;
    }
    draw();
  }
  setTimeout(tick, 500);
}
tick();
