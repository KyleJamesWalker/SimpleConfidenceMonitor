import { normalizeRoomName, roomLinks } from '/assets/shared.js';

const el = (id) => document.getElementById(id);
el('host').textContent = location.host;

const normalize = normalizeRoomName;
const urls = (room) => roomLinks(location.origin, room, el('token').value.trim());

function showLinks(room) {
  const links = urls(room);
  el('viewerUrl').value = links.viewer;
  el('consoleUrl').value = links.console;
  el('qr').src = `/api/qr?text=${encodeURIComponent(links.viewer)}`;
  el('linksPanel').hidden = false;
}

el('room').addEventListener('input', () => {
  const room = normalize(el('room').value);
  if (room) showLinks(room);
  else el('linksPanel').hidden = true;
});

el('form').addEventListener('submit', (event) => {
  event.preventDefault();
  const room = normalize(el('room').value);
  if (!room) return;
  location.href = urls(room).console;
});

el('openViewer').addEventListener('click', () => {
  const room = normalize(el('room').value);
  if (!room) return;
  window.open(urls(room).viewer, '_blank', 'noopener');
});

async function deleteRoom(room) {
  if (!window.confirm(`Delete ${room}? Any screen showing it goes blank.`)) return;
  const token = el('token').value.trim();
  const query = token ? `?token=${encodeURIComponent(token)}` : '';
  const response = await fetch(`/api/rooms/${room}${query}`, { method: 'DELETE' });
  if (!response.ok) {
    window.alert(`Could not delete ${room}: ${response.status}`);
  }
  loadRooms();
}

async function loadRooms() {
  try {
    const response = await fetch('/api/rooms');
    const { rooms } = await response.json();
    const list = el('rooms');
    list.replaceChildren();
    if (!rooms.length) {
      const empty = document.createElement('li');
      empty.className = 'hint';
      empty.textContent = 'none yet';
      list.append(empty);
      return;
    }
    for (const room of rooms) {
      const item = document.createElement('li');
      const name = document.createElement('span');
      name.className = 'name';
      name.textContent = room;
      const view = document.createElement('a');
      view.href = `/${room}`;
      view.textContent = 'viewer';
      const edit = document.createElement('a');
      edit.href = urls(room).console;
      edit.textContent = 'console';
      const remove = document.createElement('button');
      remove.type = 'button';
      remove.className = 'removeRoom';
      remove.textContent = 'delete';
      remove.addEventListener('click', () => deleteRoom(room));

      item.append(name, view, edit, remove);
      list.append(item);
    }
  } catch {
    // The list is a convenience. A failure here must not block the form.
  }
}

loadRooms();
setInterval(loadRooms, 5000);
