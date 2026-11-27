// Seaglass remote control (#336): served by the app on the local network, drives the PC's
// player. All text goes through `textContent` (titles coming from TIDAL).
"use strict";

const TEXT = {
  fr: {
    brand: "Télécommande",
    lead: "Pilotez la musique de l'ordinateur depuis ce téléphone. Scannez le QR code de Réglages › Application › Télécommande, ou saisissez le code affiché.",
    codeLabel: "Code de la télécommande",
    connect: "Se connecter",
    tabQueue: "File",
    tabSearch: "Rechercher",
    tabPlaylists: "Playlists",
    manual: "File d'attente",
    upcoming: "À suivre",
    queueEmpty: "Rien à suivre.",
    tracks: "Titres",
    albums: "Albums",
    playlists: "Playlists",
    forget: "Oublier ce code",
    nothing: "Rien en lecture",
    searchPlaceholder: "Titre, album, artiste, playlist…",
    searching: "Recherche…",
    noResults: "Rien trouvé.",
    loading: "Chargement…",
    noPlaylists: "Aucune playlist dans la bibliothèque.",
    play: "Lire",
    pause: "Pause",
    previous: "Précédent",
    next: "Suivant",
    shuffle: "Lecture aléatoire",
    repeat: "Répéter",
    mute: "Couper le son",
    remove: "Retirer de la file",
    playNext: "Lire ensuite",
    addToQueue: "Ajouter à la file",
    shufflePlay: "Lecture aléatoire",
    added: "Ajouté à la file",
    addedNext: "Lira ensuite",
    tracksCount: (n) => `${n} titre${n > 1 ? "s" : ""}`,
    errCode: "Code incorrect, ou changé depuis l'ordinateur.",
    errBlocked: "Trop d'essais : réessayez dans un quart d'heure.",
    errSlow: "Doucement : trop de commandes d'un coup.",
    errNetwork: "Ordinateur injoignable : même Wi-Fi ? Seaglass ouvert ?",
    errEmpty: "Rien de lisible ici.",
    errBackend: "L'ordinateur n'a pas pu le faire.",
  },
  en: {
    brand: "Remote",
    lead: "Control the computer's music from this phone. Scan the QR code in Settings › Application › Remote, or type the code shown there.",
    codeLabel: "Remote code",
    connect: "Connect",
    tabQueue: "Queue",
    tabSearch: "Search",
    tabPlaylists: "Playlists",
    manual: "Queue",
    upcoming: "Up next",
    queueEmpty: "Nothing up next.",
    tracks: "Tracks",
    albums: "Albums",
    playlists: "Playlists",
    forget: "Forget this code",
    nothing: "Nothing playing",
    searchPlaceholder: "Track, album, artist, playlist…",
    searching: "Searching…",
    noResults: "Nothing found.",
    loading: "Loading…",
    noPlaylists: "No playlists in the library.",
    play: "Play",
    pause: "Pause",
    previous: "Previous",
    next: "Next",
    shuffle: "Shuffle",
    repeat: "Repeat",
    mute: "Mute",
    remove: "Remove from queue",
    playNext: "Play next",
    addToQueue: "Add to queue",
    shufflePlay: "Shuffle",
    added: "Added to the queue",
    addedNext: "Plays next",
    tracksCount: (n) => `${n} track${n > 1 ? "s" : ""}`,
    errCode: "Wrong code, or changed on the computer.",
    errBlocked: "Too many attempts: try again in a quarter of an hour.",
    errSlow: "Easy: too many commands at once.",
    errNetwork: "Computer unreachable: same Wi-Fi? Seaglass open?",
    errEmpty: "Nothing playable here.",
    errBackend: "The computer could not do it.",
  },
};

const lang = (navigator.language || "en").toLowerCase().startsWith("fr") ? "fr" : "en";
const t = TEXT[lang];
document.documentElement.lang = lang;
for (const node of document.querySelectorAll("[data-t]")) node.textContent = t[node.dataset.t];

const $ = (id) => document.getElementById(id);
const CODE_KEY = "seaglass-remote-code";
const SEARCH_DELAY = 400;

let code = localStorage.getItem(CODE_KEY) || "";
let events = null;
let state = null;
/** Moment `state.position` was received, to advance the bar between two sends. */
let receivedAt = 0;
/** The finger is on a slider: the received state does not move it. */
let dragging = { position: false, volume: false };

class ApiError extends Error {
  constructor(kind) {
    super(kind);
    this.kind = kind;
  }
}

async function api(path, body) {
  let res;
  try {
    res = await fetch(`api/${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jam-Code": code },
      body: JSON.stringify(body || {}),
    });
  } catch {
    throw new ApiError("network");
  }
  const data = await res.json().catch(() => ({}));
  if (!res.ok) throw new ApiError(data.error || "backend");
  return data;
}

function message(kind) {
  return (
    {
      code: t.errCode,
      blocked: t.errBlocked,
      slow: t.errSlow,
      network: t.errNetwork,
      empty: t.errEmpty,
    }[kind] || t.errBackend
  );
}

/** Code refused (changed from the computer) or address blocked: back to the code screen. */
function lostAccess(e) {
  if (!["code", "blocked"].includes(e.kind)) return false;
  localStorage.removeItem(CODE_KEY);
  code = "";
  showJoin(message(e.kind));
  return true;
}

/** Command; the error is shown at the bottom of the screen. */
async function send(path, body) {
  try {
    return await api(path, body);
  } catch (e) {
    if (!lostAccess(e)) toast(message(e.kind));
    return null;
  }
}

let toastTimer = null;
function toast(text) {
  const node = $("toast");
  node.textContent = text;
  node.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (node.hidden = true), 3000);
}

function el(tag, className, text) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (text !== undefined) node.textContent = text;
  return node;
}

function cover(url) {
  if (!url) return el("div", "cover-empty");
  const img = el("img");
  img.src = url;
  img.alt = "";
  img.loading = "lazy";
  return img;
}

function clock(seconds) {
  const s = Math.max(0, Math.floor(seconds));
  const h = Math.floor(s / 3600);
  const m = Math.floor((s % 3600) / 60);
  const rest = String(s % 60).padStart(2, "0");
  return h ? `${h}:${String(m).padStart(2, "0")}:${rest}` : `${m}:${rest}`;
}

/** The app's "shuffle" icon, taken from the player's button. */
function shuffleIcon() {
  return $("shuffle").querySelector("svg").cloneNode(true);
}

/** `symbol`: a character, or an icon (SVG node). */
function smallButton(label, symbol, onClick) {
  const button = el("button", "small");
  button.append(symbol);
  button.type = "button";
  button.title = label;
  button.setAttribute("aria-label", label);
  button.addEventListener("click", onClick);
  return button;
}

/** List row: the main area calls `onOpen`, the buttons on the right do the rest. */
function row(item, sub, onOpen, actions = []) {
  const li = el("li", "track");
  const open = el("button", "open");
  open.type = "button";
  open.append(cover(item.cover));
  const meta = el("div", "meta");
  meta.append(el("div", "title", item.title), el("div", "sub", sub));
  open.append(meta);
  open.addEventListener("click", onOpen);
  li.append(open);
  if (actions.length) {
    const box = el("div", "actions");
    box.append(...actions);
    li.append(box);
  }
  return li;
}

// --- Connection -----------------------------------------------------------------

function showJoin(error) {
  closeEvents();
  $("remote").hidden = true;
  $("join").hidden = false;
  const err = $("join-error");
  err.hidden = !error;
  err.textContent = error || "";
  $("code").value = code;
  $("code").focus();
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
  $("join").hidden = true;
  $("remote").hidden = false;
  openEvents();
}

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
  // The browser reconnects on its own; a code changed from the computer makes it fail in a
  // loop: check it once.
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

// --- Player -------------------------------------------------------------------

function position() {
  if (!state) return 0;
  const elapsed = state.paused || state.loading ? 0 : (performance.now() - receivedAt) / 1000;
  return Math.min(state.position + elapsed, state.duration || Infinity);
}

function tick() {
  if (!state || dragging.position) return;
  const pos = position();
  $("position").value = String(Math.floor(pos));
  $("elapsed").textContent = clock(pos);
}

function render(next) {
  state = next;
  receivedAt = performance.now();
  $("player").classList.toggle("loading", next.loading);

  const now = $("now");
  now.replaceChildren();
  if (next.now) {
    now.append(cover(next.now.cover));
    const meta = el("div", "meta");
    meta.append(
      el("div", "title", next.now.title),
      el("div", "sub", next.now.artists),
      el("div", "album", next.now.album),
    );
    now.append(meta);
  } else {
    now.append(el("div", "muted", t.nothing));
  }

  const seek = $("position");
  seek.max = String(next.duration || 0);
  seek.disabled = !next.now || !next.duration;
  $("total").textContent = clock(next.duration);
  tick();

  const play = $("play");
  play.textContent = next.paused ? "▶" : "❚❚";
  play.title = next.paused ? t.play : t.pause;
  play.setAttribute("aria-label", play.title);
  play.disabled = !next.now;
  $("previous").disabled = !next.now;
  $("next").disabled = !next.hasNext;

  const shuffle = $("shuffle");
  shuffle.classList.toggle("on", next.shuffle);
  shuffle.setAttribute("aria-pressed", String(next.shuffle));
  const repeat = $("repeat");
  repeat.classList.toggle("on", next.repeat !== "off");
  repeat.classList.toggle("one", next.repeat === "one");
  repeat.setAttribute("aria-pressed", String(next.repeat !== "off"));

  const mute = $("mute");
  mute.classList.toggle("silent", next.muted || next.volume === 0);
  mute.classList.toggle("on", next.muted);
  mute.setAttribute("aria-pressed", String(next.muted));
  if (!dragging.volume) {
    $("volume").value = String(next.volume);
    $("volume-value").textContent = String(next.volume);
  }

  renderQueue(next);
}

function renderQueue(next) {
  const manual = $("manual");
  manual.replaceChildren(
    ...next.manual.map((track, index) =>
      row(track, track.artists, () => send("jump", { part: "manual", index, trackId: track.id }), [
        smallButton(t.remove, "✕", () => send("remove", { trackId: track.id })),
      ]),
    ),
  );
  const upcoming = $("upcoming");
  upcoming.replaceChildren(
    ...next.upcoming.map((track, index) =>
      row(track, track.artists, () =>
        send("jump", { part: "upcoming", index, trackId: track.id }),
      ),
    ),
  );
  $("manual-title").hidden = !next.manual.length;
  $("upcoming-title").hidden = !next.upcoming.length;
  $("queue-empty").hidden = Boolean(next.manual.length || next.upcoming.length);
}

function setupPlayer() {
  $("play").addEventListener("click", () => {
    if (!state) return;
    // Explicit, not a toggle: a double tap does not trigger the pause again.
    state.paused = !state.paused;
    send("pause", { paused: state.paused });
    render(state);
  });
  for (const [id, label] of [
    ["previous", t.previous],
    ["next", t.next],
    ["shuffle", t.shuffle],
    ["repeat", t.repeat],
    ["mute", t.mute],
  ]) {
    $(id).title = label;
    $(id).setAttribute("aria-label", label);
  }
  $("previous").addEventListener("click", () => send("previous"));
  $("next").addEventListener("click", () => send("next"));
  $("shuffle").addEventListener("click", () => state && send("shuffle", { enabled: !state.shuffle }));
  $("repeat").addEventListener("click", () => {
    if (!state) return;
    const mode = { off: "all", all: "one", one: "off" }[state.repeat] || "off";
    send("repeat", { mode });
  });
  $("mute").addEventListener("click", () => state && send("mute", { muted: !state.muted }));

  // Sliders: one command on release, not during the gesture.
  const seek = $("position");
  seek.addEventListener("input", () => {
    dragging.position = true;
    $("elapsed").textContent = clock(Number(seek.value));
  });
  seek.addEventListener("change", async () => {
    const seconds = Number(seek.value);
    if (state) {
      state.position = seconds;
      receivedAt = performance.now();
    }
    dragging.position = false;
    await send("seek", { seconds });
  });
  const volume = $("volume");
  volume.addEventListener("input", () => {
    dragging.volume = true;
    $("volume-value").textContent = volume.value;
  });
  volume.addEventListener("change", async () => {
    dragging.volume = false;
    await send("volume", { level: Number(volume.value) });
  });
  setInterval(tick, 250);
}

// --- Tabs -------------------------------------------------------------------

let playlistsLoaded = false;

function showTab(which) {
  for (const name of ["queue", "search", "playlists"]) {
    const active = name === which;
    $(`pane-${name}`).hidden = !active;
    const tab = $(`tab-${name}`);
    tab.classList.toggle("active", active);
    tab.setAttribute("aria-selected", String(active));
  }
  if (which === "search") $("query").focus();
  if (which === "playlists" && !playlistsLoaded) loadPlaylists();
}

function playlistRow(playlist) {
  const sub = playlist.tracks ? t.tracksCount(playlist.tracks) : "";
  return row(playlist, sub, () => send("play-playlist", { uuid: playlist.uuid }), [
    smallButton(t.shufflePlay, shuffleIcon(), () =>
      send("play-playlist", { uuid: playlist.uuid, shuffle: true }),
    ),
  ]);
}

async function loadPlaylists() {
  const status = $("playlists-status");
  status.hidden = false;
  status.textContent = t.loading;
  let list;
  try {
    list = await api("playlists");
  } catch (e) {
    if (!lostAccess(e)) status.textContent = message(e.kind);
    return;
  }
  playlistsLoaded = true;
  $("playlists").replaceChildren(...list.map(playlistRow));
  status.hidden = list.length > 0;
  status.textContent = t.noPlaylists;
}

// --- Search -----------------------------------------------------------------

let searchTimer = null;
let searchSeq = 0;

function scheduleSearch() {
  clearTimeout(searchTimer);
  const query = $("query").value.trim();
  $("clear").hidden = !$("query").value;
  if (!query) {
    showHits({ tracks: [], albums: [], playlists: [] });
    $("search-status").hidden = true;
    return;
  }
  searchTimer = setTimeout(() => search(query), SEARCH_DELAY);
}

async function search(query) {
  const seq = ++searchSeq;
  const status = $("search-status");
  status.hidden = false;
  status.textContent = t.searching;
  let hits;
  try {
    hits = await api("search", { query });
  } catch (e) {
    if (seq !== searchSeq) return;
    if (!lostAccess(e)) status.textContent = message(e.kind);
    return;
  }
  // A response older than the last keystroke is ignored.
  if (seq !== searchSeq) return;
  const empty = !hits.tracks.length && !hits.albums.length && !hits.playlists.length;
  status.hidden = !empty;
  status.textContent = t.noResults;
  showHits(hits);
}

function trackHit(track) {
  const added = (button, text) => {
    button.classList.add("done");
    toast(text);
  };
  const next = smallButton(t.playNext, "↪", async () => {
    if (await send("enqueue", { trackId: track.id, next: true })) added(next, t.addedNext);
  });
  const add = smallButton(t.addToQueue, "+", async () => {
    if (await send("enqueue", { trackId: track.id })) added(add, t.added);
  });
  return row(track, track.artists, () => send("play", { trackId: track.id }), [next, add]);
}

function albumHit(album) {
  const sub = [album.artists, album.year].filter(Boolean).join(" · ");
  return row(album, sub, () => send("play-album", { albumId: album.id }), [
    smallButton(t.shufflePlay, shuffleIcon(), () =>
      send("play-album", { albumId: album.id, shuffle: true }),
    ),
  ]);
}

function showHits(hits) {
  const lists = [
    ["tracks", hits.tracks.map(trackHit)],
    ["albums", hits.albums.map(albumHit)],
    ["playlists", hits.playlists.map(playlistRow)],
  ];
  for (const [name, rows] of lists) {
    $(`hits-${name}`).replaceChildren(...rows);
    $(`hits-${name}-title`).hidden = !rows.length;
  }
}

// --- Startup -----------------------------------------------------------------

$("join-form").addEventListener("submit", (event) => {
  event.preventDefault();
  join($("code").value);
});
$("forget").addEventListener("click", () => {
  localStorage.removeItem(CODE_KEY);
  code = "";
  showJoin();
});
$("query").placeholder = t.searchPlaceholder;
$("query").addEventListener("input", scheduleSearch);
$("search-form").addEventListener("submit", (event) => {
  event.preventDefault();
  clearTimeout(searchTimer);
  const query = $("query").value.trim();
  if (query) search(query);
  $("query").blur();
});
$("clear").addEventListener("click", () => {
  $("query").value = "";
  scheduleSearch();
  $("query").focus();
});
for (const name of ["queue", "search", "playlists"]) {
  $(`tab-${name}`).addEventListener("click", () => showTab(name));
}
setupPlayer();
showTab("queue");

// The app's QR code: the code comes after `#`, never sent to the server. Removed from the
// address so it does not stay in the history. A malformed fragment is ignored.
let fromQr = "";
try {
  fromQr = decodeURIComponent(location.hash.slice(1));
} catch {
  // Unreadable fragment: nothing to pick up.
}
if (location.hash) history.replaceState(null, "", location.pathname);
if (fromQr) code = fromQr;

if (code) join(code);
else showJoin();
