import { roomFromPath } from '/assets/shared.js';

const room = roomFromPath();
document.title = `${room} — operator token`;
document.getElementById('roomName').textContent = room;

// A token in the query means an attempt just failed. Say so, and take it out of
// the address bar so a refresh does not resubmit it.
const params = new URLSearchParams(location.search);
if (params.has('token')) {
  document.getElementById('failed').hidden = false;
  history.replaceState(null, '', location.pathname);
}
