import {
  MIN,
  RoomSocket,
  formatClock,
  formatDuration,
  nextClockTime,
  parseDuration,
  readout,
  roomFromPath,
  rundownTotals,
  targetOwnsKey,
} from '/assets/shared.js';

const el = (id) => document.getElementById(id);
const room = roomFromPath();
document.title = `${room} — console`;
el('roomName').textContent = room;
el('viewerLink').href = `/${room}`;
el('agendaLink').href = `/${room}/agenda`;

// The server answered with a cookie, so take the token out of the address bar
// and out of browser history.
if (new URLSearchParams(location.search).has('token')) {
  history.replaceState(null, '', location.pathname);
}

const QUICK_MINUTES = [5, 10, 15, 20, 30];
const TONES = ['neutral', 'warn', 'alert'];
const MAX_PRESETS = 8;
// The server cuts text past these, so the fields stop there instead.
const LINE_LIMIT = 120;
const NOTE_LIMIT = 500;
const MAX_NOTES = 10;
const PRESET_LIMIT = 120;

// An editor holds its own copy while open, so an arriving frame cannot
// overwrite half typed text.
let editingPresets = false;
let editingCue = null;
const TOGGLES = {
  blackout: (on) => ({ cmd: 'blackout', on }),
  showClock: (on) => ({ cmd: 'display', show_clock: on }),
  clock24h: (on) => ({ cmd: 'display', clock_24h: on }),
  showProgress: (on) => ({ cmd: 'display', show_progress: on }),
  mirror: (on) => ({ cmd: 'display', mirror: on }),
  chime: (on) => ({ cmd: 'display', chime: on }),
  showSpeaker: (on) => ({ cmd: 'display', show_speaker: on }),
  showNotes: (on) => ({ cmd: 'display', show_notes: on }),
  autoAdvance: (on) => ({ cmd: 'set_auto_advance', on }),
  auxVisible: (on) => ({ cmd: 'aux_set', visible: on }),
};
const TOGGLE_STATE = {
  blackout: (frame) => frame.display.blackout,
  showClock: (frame) => frame.display.show_clock,
  clock24h: (frame) => frame.display.clock_24h,
  showProgress: (frame) => frame.display.show_progress,
  mirror: (frame) => frame.display.mirror,
  chime: (frame) => frame.display.chime,
  showSpeaker: (frame) => frame.display.show_speaker,
  showNotes: (frame) => frame.display.show_notes,
  autoAdvance: (frame) => frame.rundown.auto_advance,
  auxVisible: (frame) => frame.aux.visible,
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
  onRefused: () => {
    toast('The operator token expired. Opening the token form.');
    setTimeout(() => {
      location.href = `/${room}/edit`;
    }, 1500);
  },
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

for (const id of ['title', 'nextUp', 'message', 'warn', 'danger', 'duration', 'startAt', 'auxLabel']) {
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
  syncField('auxLabel', frame.aux.label);
  if (!editing('scale')) {
    el('scale').value = display.scale;
    el('scaleOut').textContent = `${display.scale}%`;
  }

  tone = message.tone;
  for (const button of document.querySelectorAll('.tone')) {
    pressed(button, button.dataset.tone === tone);
  }
  for (const [id, read] of Object.entries(TOGGLE_STATE)) {
    pressed(el(id), Boolean(read(frame)));
  }
  pressed(el('showMessage'), message.visible && Boolean(message.text));
  drawArmed(frame);
  drawPresets(frame);
  drawRundown(frame);
  render();
}

// A CSS class alone leaves a screen reader unable to tell whether blackout is
// live, so the state rides on aria-pressed too.
function pressed(node, on) {
  node.classList.toggle('on', on);
  node.setAttribute('aria-pressed', String(Boolean(on)));
}

function drawArmed(frame) {
  const at = frame.timer.start_at_ms;
  el('armed').hidden = !at;
  if (at) {
    const clock = formatClock(new Date(at), frame.display.clock_24h);
    el('armed').textContent = `Starts at ${clock}`;
  }
}

function drawPresets(frame) {
  const row = el('presets');
  if (editingPresets) return;
  const signature = frame.presets.map((preset) => `${preset.tone}:${preset.text}`).join('|');
  if (row.dataset.signature === signature) return;
  row.dataset.signature = signature;
  row.replaceChildren();
  for (const [index, preset] of frame.presets.entries()) {
    const button = document.createElement('button');
    button.type = 'button';
    button.className = `preset tone-${preset.tone}`;
    button.textContent = preset.text;
    button.title = `Send: ${preset.text}`;
    button.addEventListener('click', () => send({ cmd: 'send_preset', index }));
    row.append(button);
  }
}

function drawRundown(frame) {
  const { rundown } = frame;
  const list = el('cues');
  if (editingCue !== null && rundown.cues.some((cue) => cue.id === editingCue)) return;
  editingCue = null;
  list.replaceChildren();
  for (const [index, cue] of rundown.cues.entries()) {
    const item = document.createElement('li');
    if (cue.id === rundown.active) item.classList.add('active');

    const position = document.createElement('span');
    position.className = 'index';
    position.textContent = index + 1;

    const label = document.createElement('span');
    label.textContent = cue.title || '(untitled)';
    if (cue.speaker) {
      const who = document.createElement('span');
      who.className = 'who';
      who.textContent = ` ${cue.speaker}`;
      label.append(who);
    }

    const length = document.createElement('span');
    length.className = 'len';
    length.textContent = formatDuration(cue.duration_ms);

    // Notes live behind Edit, so the row says whether there are any.
    const count = cue.notes?.length || 0;
    if (count) {
      const badge = document.createElement('span');
      badge.className = 'noteCount';
      badge.textContent = `${count} note${count === 1 ? '' : 's'}`;
      badge.title = cue.notes.map((note) => note.text).join('\n');
      length.append(badge);
    }

    const actions = document.createElement('span');
    actions.className = 'actions';
    const named = cue.title || `cue ${index + 1}`;
    actions.append(
      action('Load', () => send({ cmd: 'load_cue', id: cue.id }), `Load ${named}`),
      action('Edit', () => openCueEditor(item, cue), `Edit ${named}`),
      action(
        'Up',
        () => send({ cmd: 'move_cue', id: cue.id, to: Math.max(0, index - 1) }),
        `Move ${named} up`,
      ),
      action('Down', () => send({ cmd: 'move_cue', id: cue.id, to: index + 1 }), `Move ${named} down`),
      action('X', () => removeCue(cue, named), `Remove ${named}`),
    );

    item.append(position, label, length, actions);
    list.append(item);
  }

  const totals = rundownTotals(rundown, readout(frame.timer, socket.serverNow()).remainingMs);
  el('totals').textContent = totals.cueCount
    ? `${totals.cueCount} cues · ${formatDuration(totals.remainingMs)} left of ${formatDuration(totals.totalMs)}`
    : '';
}

// One row turns into a form in place. Save sends only what changed.
function openCueEditor(row, cue) {
  editingCue = cue.id;
  const form = document.createElement('form');
  form.className = 'cueEdit';

  const field = (label, value, extra = {}) => {
    const wrap = document.createElement('label');
    wrap.className = 'field';
    const name = document.createElement('span');
    name.textContent = label;
    const input = document.createElement('input');
    input.type = 'text';
    input.value = value;
    Object.assign(input, extra);
    wrap.append(name, input);
    return { wrap, input };
  };

  const title = field('Title', cue.title, { maxLength: LINE_LIMIT });
  const speaker = field('Speaker', cue.speaker, { maxLength: LINE_LIMIT });
  const length = field('Length', formatDuration(cue.duration_ms), {
    inputMode: 'numeric',
    placeholder: '5:00',
  });
  const notes = noteEditor(cue.notes);

  const buttons = document.createElement('div');
  buttons.className = 'transport';
  const save = document.createElement('button');
  save.type = 'submit';
  save.className = 'primary';
  save.textContent = 'Save';
  const cancel = document.createElement('button');
  cancel.type = 'button';
  cancel.textContent = 'Cancel';
  cancel.addEventListener('click', () => {
    editingCue = null;
    if (state) {
      el('cues').replaceChildren();
      drawRundown(state);
    }
  });
  buttons.append(save, cancel);

  form.append(title.wrap, speaker.wrap, length.wrap, notes.wrap, buttons);

  form.addEventListener('submit', (event) => {
    event.preventDefault();
    const ms = parseDuration(length.input.value);
    if (ms === null) {
      toast('Cue length takes minutes, or mm:ss');
      return;
    }
    const written = notes.read();
    if (written === null) {
      toast('A note time takes minutes, or mm:ss');
      return;
    }
    send({
      cmd: 'update_cue',
      id: cue.id,
      title: title.input.value.trim(),
      speaker: speaker.input.value.trim(),
      duration_ms: ms,
      notes: written,
    });
    editingCue = null;
  });

  row.replaceChildren(form);
  title.input.focus();
}

// Delete sits in a dense row beside Load and Down, mid-show, and there is no
// undo. Clearing a room and deleting one both ask first.
function removeCue(cue, named) {
  const onAir = state?.rundown.active === cue.id;
  const consequence = onAir ? ' It is on air, so the stage title and next up go blank.' : '';
  if (!window.confirm(`Remove ${named} from the rundown?${consequence}`)) return;
  send({ cmd: 'remove_cue', id: cue.id });
}

// A cue's notes, as one row per note: when it starts, and what it says. The
// time is minutes into the cue, which is how a rundown is written. The screen
// turns it into time remaining.
function noteEditor(notes) {
  const wrap = document.createElement('div');
  wrap.className = 'noteEdit';

  const heading = document.createElement('span');
  heading.className = 'noteHeading';
  heading.textContent = 'Notes for the speaker (time into the cue)';

  const rows = document.createElement('div');
  rows.className = 'noteRows';

  const add = document.createElement('button');
  add.type = 'button';
  add.className = 'addNote';
  add.textContent = 'Add note';
  add.addEventListener('click', () => {
    if (rows.children.length >= MAX_NOTES) {
      toast(`A cue holds at most ${MAX_NOTES} notes`);
      return;
    }
    const row = noteRow({ at_ms: suggestedAt(rows), text: '' });
    rows.append(row);
    row.querySelector('.noteText').focus();
  });

  rows.replaceChildren(...(notes || []).map(noteRow));
  wrap.append(heading, rows, add);

  // null when a time does not parse, so the caller can say so and keep the form.
  const read = () => {
    const written = [];
    for (const row of rows.children) {
      const text = row.querySelector('.noteText').value.trim();
      if (!text) continue;
      const at = parseDuration(row.querySelector('.noteAt').value || '0');
      if (at === null) return null;
      written.push({ at_ms: at, text });
    }
    return written.sort((left, right) => left.at_ms - right.at_ms);
  };

  return { wrap, read };
}

function noteRow(note) {
  const row = document.createElement('div');
  row.className = 'noteRow';

  const at = document.createElement('input');
  at.type = 'text';
  at.className = 'noteAt';
  at.inputMode = 'numeric';
  at.placeholder = '0:00';
  at.value = formatDuration(note.at_ms || 0);
  at.setAttribute('aria-label', 'Minutes into the cue');

  const text = document.createElement('textarea');
  text.className = 'noteText';
  text.rows = 2;
  text.maxLength = NOTE_LIMIT;
  text.placeholder = 'What the speaker should read';
  text.value = note.text || '';
  text.setAttribute('aria-label', 'Note text');

  const remove = document.createElement('button');
  remove.type = 'button';
  remove.className = 'removeNote';
  remove.textContent = '\u00d7';
  remove.title = 'Remove this note';
  remove.setAttribute('aria-label', 'Remove this note');
  remove.addEventListener('click', () => row.remove());

  row.append(at, text, remove);
  return row;
}

// A new note lands after the last one, which is where an author is working.
function suggestedAt(rows) {
  const times = [...rows.children]
    .map((row) => parseDuration(row.querySelector('.noteAt').value || '0'))
    .filter((ms) => ms !== null);
  return times.length ? Math.max(...times) + 5 * MIN : 0;
}

function action(label, onClick, description = label) {
  const button = document.createElement('button');
  button.type = 'button';
  button.textContent = label;
  button.title = description;
  button.setAttribute('aria-label', description);
  button.addEventListener('click', onClick);
  return button;
}

function render() {
  if (state) {
    const now = socket.serverNow();
    const auxOut = readout(state.aux.timer, now);
    const auxText = formatDuration(auxOut.valueMs);
    if (painted.aux !== auxText) {
      el('auxReadout').textContent = auxText;
      el('auxReadout').className = `auxReadout ${auxOut.phase}`;
      painted.aux = auxText;
    }
    const out = readout(state.timer, now);
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

el('exportCsv').href = `/api/rooms/${room}/rundown.csv`;
el('exportJson').href = `/api/rooms/${room}/rundown.json`;

el('importFile').addEventListener('change', async (event) => {
  const file = event.target.files?.[0];
  if (!file) return;
  const isJson = file.name.endsWith('.json');
  try {
    const response = await fetch(`/api/rooms/${room}/rundown`, {
      method: 'POST',
      headers: { 'content-type': isJson ? 'application/json' : 'text/csv' },
      body: await file.text(),
    });
    if (!response.ok) {
      toast(`Import failed: ${await response.text()}`);
    }
  } catch (error) {
    toast(`Import failed: ${error.message}`);
  }
  event.target.value = '';
});

function presetRow(preset) {
  const row = document.createElement('div');
  row.className = 'presetRow';

  const text = document.createElement('input');
  text.type = 'text';
  text.maxLength = PRESET_LIMIT;
  text.className = 'presetText';
  text.value = preset.text;
  text.placeholder = 'Message';

  const tone = document.createElement('select');
  tone.className = 'presetTone';
  for (const name of TONES) {
    const option = document.createElement('option');
    option.value = name;
    option.textContent = name;
    tone.append(option);
  }
  tone.value = preset.tone;

  const remove = document.createElement('button');
  remove.type = 'button';
  remove.className = 'removePreset';
  remove.textContent = '\u00d7';
  remove.title = 'Remove';
  remove.setAttribute('aria-label', `Remove preset: ${preset.text || 'new message'}`);
  remove.addEventListener('click', () => row.remove());

  row.append(text, tone, remove);
  return row;
}

function openPresetEditor() {
  editingPresets = true;
  el('presetRows').replaceChildren(...(state?.presets || []).map(presetRow));
  el('presetEditor').hidden = false;
  el('presets').hidden = true;
  pressed(el('editPresets'), true);
}

function closePresetEditor() {
  editingPresets = false;
  el('presetEditor').hidden = true;
  el('presets').hidden = false;
  pressed(el('editPresets'), false);
  el('presets').dataset.signature = '';
  if (state) drawPresets(state);
}

el('editPresets').addEventListener('click', () => {
  if (editingPresets) closePresetEditor();
  else openPresetEditor();
});

el('addPreset').addEventListener('click', () => {
  if (el('presetRows').children.length >= MAX_PRESETS) {
    toast(`A room holds at most ${MAX_PRESETS} presets`);
    return;
  }
  const row = presetRow({ text: 'New message', tone: 'neutral' });
  el('presetRows').append(row);
  row.querySelector('.presetText').focus();
});

el('cancelPresets').addEventListener('click', closePresetEditor);

el('savePresets').addEventListener('click', () => {
  const presets = [...el('presetRows').children]
    .map((row) => ({
      text: row.querySelector('.presetText').value.trim(),
      tone: row.querySelector('.presetTone').value,
    }))
    .filter((preset) => preset.text);
  send({ cmd: 'set_presets', presets });
  closePresetEditor();
});

el('clearRoom').addEventListener('click', () => {
  if (!window.confirm(`Clear everything in ${room}?`)) return;
  send({ cmd: 'clear_room' });
});

el('auxStart').addEventListener('click', () => send({ cmd: 'aux_start' }));
el('auxPause').addEventListener('click', () => send({ cmd: 'aux_pause' }));
el('auxReset').addEventListener('click', () => send({ cmd: 'aux_reset' }));
el('auxLabel').addEventListener('input', (event) => {
  const label = event.target.value;
  clearTimeout(el('auxLabel').timer);
  el('auxLabel').timer = setTimeout(() => send({ cmd: 'aux_set', label }), 250);
});

for (const minutes of [5, 10, 15]) {
  const button = document.createElement('button');
  button.type = 'button';
  button.textContent = `${minutes}m`;
  button.addEventListener('click', () => send({ cmd: 'aux_set_duration', ms: minutes * MIN }));
  el('auxChips').append(button);
}

el('arm').addEventListener('click', () => {
  const at = nextClockTime(el('startAt').value, socket.serverNow());
  if (at === null) {
    toast('Start time takes hh:mm on a 24 hour clock');
    return;
  }
  send({ cmd: 'schedule_start', at_ms: at });
  el('startAt').value = '';
  drafts.delete('startAt');
});

el('disarm').addEventListener('click', () => send({ cmd: 'schedule_start', at_ms: null }));

el('nextCue').addEventListener('click', () => send({ cmd: 'next_cue' }));
el('nextAndStart').addEventListener('click', () => send({ cmd: 'next_and_start' }));
el('prevCue').addEventListener('click', () => send({ cmd: 'prev_cue' }));

el('cueForm').addEventListener('submit', (event) => {
  event.preventDefault();
  const minutes = el('cueMinutes').value.trim();
  const duration = minutes ? parseDuration(minutes) : 5 * MIN;
  if (duration === null) {
    toast('Cue length takes minutes, or mm:ss');
    return;
  }
  send({
    cmd: 'add_cue',
    title: el('cueTitle').value.trim(),
    speaker: el('cueSpeaker').value.trim(),
    duration_ms: duration,
  });
  for (const id of ['cueTitle', 'cueSpeaker', 'cueMinutes']) {
    el(id).value = '';
  }
  el('cueTitle').focus();
});

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

// A mouse click leaves the button focused, and Space is the transport's key.
// The operator already clicked it, so hand focus back to the page. detail is
// the click count, so a keyboard activation keeps its focus and stays tabbable.
document.addEventListener('click', (event) => {
  if (event.detail > 0) event.target.closest?.('button')?.blur();
});

document.addEventListener('keydown', (event) => {
  const tabbedTo = event.target.matches?.(':focus-visible') ?? false;
  const owned = targetOwnsKey(event.target.tagName, event.key, tabbedTo);
  if (owned || event.metaKey || event.ctrlKey || event.altKey) return;
  const running = state?.timer.run.state === 'running';
  const keys = {
    ' ': () => send({ cmd: running ? 'pause' : 'start' }),
    r: () => send({ cmd: 'reset' }),
    b: () => send({ cmd: 'blackout', on: !state?.display.blackout }),
    f: () => send({ cmd: 'flash' }),
    n: () => send({ cmd: 'next_cue' }),
    p: () => send({ cmd: 'prev_cue' }),
    g: () => send({ cmd: 'next_and_start' }),
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
