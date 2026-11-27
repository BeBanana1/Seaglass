// --- Join ---------------------------------------------------------------

function showJoin(error) {
  closeEvents();
  $("jam").hidden = true;
  $("join").hidden = false;
  const err = $("join-error");
  err.hidden = !error;
  err.textContent = error || "";
  $("name").value = name;
  $("code").value = code;
  (name ? $("code") : $("name")).focus();
}

async function join(candidate) {
  code = candidate.replace(/[\s-]/g, "").toUpperCase();
  try {
    await api("join");
  } catch (e) {
    if (!lostAccess(e)) showJoin(message(e.kind));
    return;
  }
  localStorage.setItem(CODE_KEY, code);
  loadMine();
  loadPlayer();
  $("join").hidden = true;
  $("jam").hidden = false;
  showWho();
  openEvents();
}

function showWho() {
  $("who").textContent = name ? t.who(name) : t.anonymous;
}

// --- Live state ---------------------------------------------------------

function render(state) {
  const now = $("now");
  now.replaceChildren();
  if (state.now) {
    now.append(cover(state.now.cover));
    const meta = el("div", "meta");
    meta.append(
      el("div", "label", state.paused ? t.paused : t.nowPlaying),
      el("div", "title", state.now.title),
      el("div", "sub", state.now.artists),
    );
    // Lyrics (#448): hidden during a blind test, or when the host knows there are none.
    if (!state.blind && state.lyrics !== false) {
      const open = el("button", "link lyrics-open", t.lyrics);
      open.type = "button";
      open.addEventListener("click", openLyrics);
      meta.append(open);
    }
    now.append(meta);
    if (state.controls) now.append(pauseButton(state.paused));
  } else {
    now.append(el("div", "muted", t.nothing));
  }
  renderVolume(state.volume);
  renderSkip(state);
  followLyrics(state);
  // Host rules (#288): additions suspended, limit per guest, albums.
  const notice = $("notice");
  const lines = [];
  if (state.blind) lines.push(t.gameNotice);
  else if (state.adds === false) lines.push(t.closed);
  if (state.limit) lines.push(t.limit(state.limit));
  notice.textContent = lines.join(" ");
  notice.hidden = lines.length === 0;
  notice.classList.toggle("warn", state.adds === false && !state.blind);
  renderGame(state.blind);
  const albumsChanged = lastState && lastState.albums !== state.albums;
  // The host has taken into account the pending additions it sends back.
  for (const track of state.queue) pending.delete(track.id);
  lastState = state;
  const rows = [];
  const headed = new Set();
  for (const track of state.queue) {
    const kind = track.batch === undefined ? undefined : mine[track.batch];
    // An album added from this browser: a header before its first track.
    if (kind === "album" && !headed.has(track.batch)) {
      headed.add(track.batch);
      rows.push(albumHead(track));
    }
    rows.push(trackRow(track, kind ? removeButton(track) : undefined));
  }
  $("queue").replaceChildren(...rows);
  $("queue-empty").hidden = state.queue.length > 0;
  if (albumsChanged) showResults();
}

function albumHead(track) {
  const li = el("li", "batch-head");
  li.append(el("span", "batch-title", t.yourAlbum(track.album)));
  const button = el("button", "link", t.removeAlbum);
  button.type = "button";
  button.addEventListener("click", () => removeAdded(button, { batch: track.batch }));
  li.append(button);
  return li;
}

function removeButton(track) {
  const button = el("button", "remove", "✕");
  button.type = "button";
  button.setAttribute("aria-label", `${t.remove} : ${track.title}`);
  button.addEventListener("click", () =>
    removeAdded(button, { batch: track.batch, trackId: track.id }),
  );
  return button;
}

/** Removes a track or a whole batch added from this browser; the queue comes back via the
 * stream. */
async function removeAdded(button, target) {
  button.disabled = true;
  let res;
  try {
    res = await api("remove", { guest, ...target });
  } catch (e) {
    button.disabled = false;
    if (!lostAccess(e)) toast(message(e.kind));
    return;
  }
  if (target.trackId === undefined) toast(t.albumRemoved(res.removed || 0));
  else toast(t.removed);
}

/** Play / pause (#222), if the host allows it. The real state comes back via the stream. */
function pauseButton(paused) {
  const button = el("button", "play-pause", paused ? "▶︎" : "❚❚");
  button.type = "button";
  button.setAttribute("aria-label", paused ? t.play : t.pause);
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await api("pause", { paused: !paused });
    } catch (e) {
      button.disabled = false;
      if (!lostAccess(e)) toast(message(e.kind));
    }
  });
  return button;
}

/** Volume (#449), if the host allows it: the slider stops at the host's ceiling, and is not
 * moved by the pushed state while the guest is holding it. */
let volumeDragging = false;
function renderVolume(volume) {
  const box = $("guest-volume");
  box.hidden = !volume;
  if (!volume || volumeDragging) return;
  const input = $("volume");
  input.max = String(volume.max);
  input.value = String(Math.min(volume.level, volume.max));
  input.disabled = volume.muted;
  $("volume-value").textContent = volume.muted ? "—" : String(volume.level);
}

async function sendVolume() {
  const input = $("volume");
  volumeDragging = false;
  try {
    await api("volume", { level: Number(input.value) });
  } catch (e) {
    if (!lostAccess(e)) toast(message(e.kind));
  }
  // The real state comes back via the stream; if refused, the slider goes back.
  if (lastState) renderVolume(lastState.volume);
}

// --- Vote to skip (#433) ---------------------------------------------------------
// One vote per track: the page remembers the track it voted for, the count comes back
// via the stream.

const SKIP_KEY = "seaglass-jam-skip";
let skipVotedFor = Number(sessionStorage.getItem(SKIP_KEY)) || null;

function renderSkip(state) {
  const box = $("guest-skip");
  const skip = state.skip;
  box.hidden = !skip || !state.now;
  if (box.hidden) return;
  $("skip-count").textContent = t.skipCount(skip.votes, skip.needed);
  const button = $("skip");
  const voted = skipVotedFor === state.now.id;
  button.textContent = voted ? t.skipVoted : t.skip;
  button.disabled = voted;
}

async function voteSkip() {
  const trackId = lastState && lastState.now && lastState.now.id;
  if (!trackId) return;
  const button = $("skip");
  button.disabled = true;
  let res;
  try {
    res = await api("skip", { trackId });
  } catch (e) {
    button.disabled = false;
    if (!lostAccess(e)) toast(message(e.kind));
    return;
  }
  skipVotedFor = trackId;
  sessionStorage.setItem(SKIP_KEY, String(trackId));
  if (res.skipped) toast(t.skipped);
  if (lastState) renderSkip(lastState);
}

// --- Event stream -------------------------------------------------------------

function openEvents() {
  closeEvents();
  events = new EventSource(`api/events?code=${encodeURIComponent(code)}`);
  events.onmessage = (event) => {
    try {
      render(JSON.parse(event.data));
    } catch {
      // Unreadable message: the next one will replace it.
    }
  };
  // The browser reconnects on its own; a code that became invalid (Jam restarted) makes it
  // fail in a loop: check it once.
  events.onerror = async () => {
    try {
      await api("join");
    } catch (e) {
      lostAccess(e);
    }
  };
}

function closeEvents() {
  events?.close();
  events = null;
}
