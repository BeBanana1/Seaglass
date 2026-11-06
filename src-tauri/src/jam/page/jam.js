// Page des invités de la Jam (#25). Aucune donnée distante n'entre dans le HTML :
// tout passe par textContent.
"use strict";

const TEXT = {
  fr: {
    lead: "Ajoutez des titres à la file de l'hôte.",
    nameLabel: "Votre prénom (affiché à côté de vos titres)",
    codeLabel: "Code de la Jam",
    join: "Rejoindre",
    searchPlaceholder: "Chercher un titre, un artiste…",
    clear: "Effacer la recherche",
    searching: "Recherche…",
    upNext: "À suivre",
    queueEmpty: "Rien dans la file pour l'instant.",
    nowPlaying: "En lecture",
    paused: "En pause",
    nothing: "Rien en lecture",
    play: "Lecture",
    pause: "Pause",
    noResults: "Aucun titre trouvé.",
    add: "Ajouter à la file",
    addAlbum: "Ajouter l'album à la file",
    added: (t) => `« ${t} » ajouté à la file`,
    albumAdded: (t, n) => `« ${t} » ajouté à la file (${n} titre${n > 1 ? "s" : ""})`,
    tabTracks: (n) => `Titres (${n})`,
    tabAlbums: (n) => `Albums (${n})`,
    albumTracks: (n) => `${n} titre${n > 1 ? "s" : ""}`,
    remove: "Retirer de la file",
    removed: "Retiré de la file",
    yourAlbum: (t) => `Votre album : ${t}`,
    removeAlbum: "Retirer l'album",
    albumRemoved: (n) => `${n} titre${n > 1 ? "s" : ""} retiré${n > 1 ? "s" : ""} de la file`,
    by: (name) => `ajouté par ${name}`,
    who: (name) => `Vous êtes ${name}.`,
    anonymous: "Vous ajoutez des titres sans nom.",
    rename: "Changer",
    errCode: "Code incorrect.",
    errBlocked: "Trop d'essais : demandez à l'hôte de relancer la Jam.",
    errSlow: "Doucement : réessayez dans une minute.",
    errNetwork: "La Jam ne répond pas. Elle est peut-être terminée.",
    errBackend: "L'hôte n'a pas pu répondre. Réessayez.",
    errUnknown: "Ce titre n'est plus disponible : relancez la recherche.",
    errOffline: "L'hôte a perdu le réseau. Il devrait revenir dans un instant.",
    errEnded: "Cette Jam est terminée.",
    errControls: "L'hôte a désactivé la pause pour les invités.",
    errEmpty: "Cet album n'a aucun titre disponible.",
  },
  en: {
    lead: "Add tracks to the host's queue.",
    nameLabel: "Your first name (shown next to your tracks)",
    codeLabel: "Jam code",
    join: "Join",
    searchPlaceholder: "Search for a track or artist…",
    clear: "Clear search",
    searching: "Searching…",
    upNext: "Up next",
    queueEmpty: "Nothing in the queue yet.",
    nowPlaying: "Now playing",
    paused: "Paused",
    nothing: "Nothing playing",
    play: "Play",
    pause: "Pause",
    noResults: "No tracks found.",
    add: "Add to queue",
    addAlbum: "Add the album to the queue",
    added: (t) => `“${t}” added to the queue`,
    albumAdded: (t, n) => `“${t}” added to the queue (${n} track${n > 1 ? "s" : ""})`,
    tabTracks: (n) => `Tracks (${n})`,
    tabAlbums: (n) => `Albums (${n})`,
    albumTracks: (n) => `${n} track${n > 1 ? "s" : ""}`,
    remove: "Remove from the queue",
    removed: "Removed from the queue",
    yourAlbum: (t) => `Your album: ${t}`,
    removeAlbum: "Remove the album",
    albumRemoved: (n) => `${n} track${n > 1 ? "s" : ""} removed from the queue`,
    by: (name) => `added by ${name}`,
    who: (name) => `You are ${name}.`,
    anonymous: "You add tracks without a name.",
    rename: "Change",
    errCode: "Wrong code.",
    errBlocked: "Too many attempts: ask the host to restart the Jam.",
    errSlow: "Slow down: try again in a minute.",
    errNetwork: "The Jam isn't responding. It may have ended.",
    errBackend: "The host couldn't answer. Try again.",
    errUnknown: "This track is no longer available: search again.",
    errOffline: "The host lost their connection. They should be back shortly.",
    errEnded: "This Jam has ended.",
    errControls: "The host has turned off pausing for guests.",
    errEmpty: "This album has no available tracks.",
  },
};
const lang = (navigator.language || "en").toLowerCase().startsWith("fr") ? "fr" : "en";
const t = TEXT[lang];
document.documentElement.lang = lang;

const $ = (id) => document.getElementById(id);
const CODE_KEY = "seaglass-jam-code";
const NAME_KEY = "seaglass-jam-name";
const GUEST_KEY = "seaglass-jam-guest";
const MINE_KEY = "seaglass-jam-mine";
const SEARCH_DELAY = 400;

let code = localStorage.getItem(CODE_KEY) || "";
let name = localStorage.getItem(NAME_KEY) || "";
/** Flux de l'état poussé par l'hôte. */
let events = null;
/** Titres ajoutés depuis cette page, montrés tout de suite sans attendre l'hôte. */
const pending = new Set();
const pendingAlbums = new Set();

/** Jeton de ce navigateur (#223) : l'hôte ne laisse retirer un ajout qu'à son auteur.
 * `getRandomValues` existe aussi en HTTP simple (réseau local), pas `randomUUID`. */
const guest =
  localStorage.getItem(GUEST_KEY) ||
  (() => {
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    const token = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    localStorage.setItem(GUEST_KEY, token);
    return token;
  })();

/** Lots ajoutés depuis ce navigateur pendant la Jam en cours : `{ <lot>: "track" | "album" }`. */
let mine = {};
function loadMine() {
  try {
    const saved = JSON.parse(localStorage.getItem(MINE_KEY) || "{}");
    mine = saved.code === code && saved.batches ? saved.batches : {};
  } catch {
    mine = {};
  }
}
function rememberBatch(batch, kind) {
  if (typeof batch !== "number") return;
  mine[batch] = kind;
  localStorage.setItem(MINE_KEY, JSON.stringify({ code, batches: mine }));
  if (lastState) render(lastState);
}
let lastState = null;

class ApiError extends Error {
  constructor(kind) {
    super(kind);
    this.kind = kind;
  }
}

async function api(path, body) {
  let res;
  try {
    // Chemins relatifs : la page est servie à `/` (réseau local) ou à `/j/<id>/` (relais).
    res = await fetch(`api/${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jam-Code": code },
      body: body === undefined ? "" : JSON.stringify(body),
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
      unknown: t.errUnknown,
      offline: t.errOffline,
      busy: t.errOffline,
      ended: t.errEnded,
      controls: t.errControls,
      empty: t.errEmpty,
    }[kind] || t.errBackend
  );
}

/** Code refusé, adresse bloquée ou Jam terminée (relais) : retour à l'écran pour rejoindre. */
function lostAccess(e) {
  if (e.kind !== "code" && e.kind !== "blocked" && e.kind !== "ended") return false;
  localStorage.removeItem(CODE_KEY);
  code = "";
  showJoin(message(e.kind));
  return true;
}

let toastTimer = null;
function toast(text) {
  const el = $("toast");
  el.textContent = text;
  el.hidden = false;
  clearTimeout(toastTimer);
  toastTimer = setTimeout(() => (el.hidden = true), 3000);
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

function trackRow(track, action) {
  const li = el("li", "track");
  li.append(cover(track.cover));
  const meta = el("div", "meta");
  meta.append(el("div", "title", track.title), el("div", "sub", track.artists));
  if (track.by) meta.append(el("div", "by", t.by(track.by)));
  li.append(meta);
  if (action) li.append(action);
  return li;
}

// --- Rejoindre ---------------------------------------------------------------

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
  $("join").hidden = true;
  $("jam").hidden = false;
  showWho();
  openEvents();
}

function showWho() {
  $("who").textContent = name ? t.who(name) : t.anonymous;
}

// --- État en direct ---------------------------------------------------------

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
    now.append(meta);
    if (state.controls) now.append(pauseButton(state.paused));
  } else {
    now.append(el("div", "muted", t.nothing));
  }
  // L'hôte a pris en compte les ajouts en attente qu'il renvoie.
  for (const track of state.queue) pending.delete(track.id);
  lastState = state;
  const rows = [];
  const headed = new Set();
  for (const track of state.queue) {
    const kind = track.batch === undefined ? undefined : mine[track.batch];
    // Un album ajouté depuis ce navigateur : un en-tête avant son premier titre.
    if (kind === "album" && !headed.has(track.batch)) {
      headed.add(track.batch);
      rows.push(albumHead(track));
    }
    rows.push(trackRow(track, kind ? removeButton(track) : undefined));
  }
  $("queue").replaceChildren(...rows);
  $("queue-empty").hidden = state.queue.length > 0;
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

/** Retire un titre ou tout un lot ajouté depuis ce navigateur ; la file revient par le flux. */
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

/** Lecture / pause (#222), si l'hôte le permet. L'état réel revient par le flux. */
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

function openEvents() {
  closeEvents();
  events = new EventSource(`api/events?code=${encodeURIComponent(code)}`);
  events.onmessage = (event) => {
    try {
      render(JSON.parse(event.data));
    } catch {
      // Message illisible : le suivant le remplacera.
    }
  };
  // Le navigateur se reconnecte seul ; un code devenu invalide (Jam relancée) le fait
  // échouer en boucle : on le vérifie une fois.
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

// --- Recherche et ajout ---------------------------------------------------------

let searchTimer = null;
let searchSeq = 0;

function scheduleSearch() {
  clearTimeout(searchTimer);
  searchTimer = setTimeout(() => search($("query").value), SEARCH_DELAY);
}

async function search(query) {
  clearTimeout(searchTimer);
  const seq = ++searchSeq;
  const results = $("results");
  const status = $("search-status");
  if (!query.trim()) {
    status.hidden = true;
    clearResults();
    return;
  }
  status.textContent = t.searching;
  status.hidden = false;
  results.classList.add("stale");
  let hits;
  try {
    hits = await api("search", { query, albums: true });
  } catch (e) {
    if (seq !== searchSeq) return;
    status.hidden = true;
    results.classList.remove("stale");
    if (!lostAccess(e)) toast(message(e.kind));
    return;
  }
  // Une recherche plus récente est partie entre-temps.
  if (seq !== searchSeq) return;
  // Un hôte d'avant #223 ne renvoie que les titres.
  lastHits = Array.isArray(hits) ? { tracks: hits, albums: [] } : hits;
  results.classList.remove("stale");
  showResults();
}

let lastHits = { tracks: [], albums: [] };
let tab = "tracks";

function showResults() {
  const { tracks, albums } = lastHits;
  const status = $("search-status");
  const any = tracks.length + albums.length > 0;
  status.hidden = any;
  status.textContent = any ? "" : t.noResults;
  // Onglets seulement s'il y a des albums ; sinon, les titres.
  $("tabs").hidden = albums.length === 0;
  if (albums.length === 0) tab = "tracks";
  if (tab === "tracks" && tracks.length === 0 && albums.length > 0) tab = "albums";
  for (const [id, which, count] of [
    ["tab-tracks", "tracks", t.tabTracks(tracks.length)],
    ["tab-albums", "albums", t.tabAlbums(albums.length)],
  ]) {
    const button = $(id);
    button.textContent = count;
    button.setAttribute("aria-selected", String(tab === which));
    button.classList.toggle("active", tab === which);
  }
  $("results").replaceChildren(
    ...(tab === "albums"
      ? albums.map((album) => albumRow(album))
      : tracks.map((track) => trackRow(track, addButton(track)))),
  );
}

function clearResults() {
  lastHits = { tracks: [], albums: [] };
  $("tabs").hidden = true;
  $("results").replaceChildren();
}

function albumRow(album) {
  const li = el("li", "track");
  li.append(cover(album.cover));
  const meta = el("div", "meta");
  const details = [album.artists, album.year, album.tracks ? t.albumTracks(album.tracks) : null];
  meta.append(el("div", "title", album.title), el("div", "sub", details.filter(Boolean).join(" · ")));
  li.append(meta);
  const button = el("button", "add", "+");
  button.type = "button";
  button.setAttribute("aria-label", `${t.addAlbum} : ${album.title}`);
  if (pendingAlbums.has(album.id)) markAdded(button, true);
  button.addEventListener("click", async () => {
    markAdded(button, true);
    pendingAlbums.add(album.id);
    let res;
    try {
      res = await api("add-album", { albumId: album.id, name: name || undefined, guest });
    } catch (e) {
      pendingAlbums.delete(album.id);
      markAdded(button, false);
      if (!lostAccess(e)) toast(message(e.kind));
      return;
    }
    rememberBatch(res.batch, "album");
    toast(t.albumAdded(album.title, res.count || 0));
  });
  li.append(button);
  return li;
}

function markAdded(button, added) {
  button.textContent = added ? "✓" : "+";
  button.classList.toggle("done", added);
  button.disabled = added;
}

function addButton(track) {
  const button = el("button", "add", "+");
  button.type = "button";
  button.setAttribute("aria-label", `${t.add} : ${track.title}`);
  if (pending.has(track.id)) markAdded(button, true);
  button.addEventListener("click", async () => {
    // Affiché aussitôt, annulé si l'hôte refuse.
    markAdded(button, true);
    pending.add(track.id);
    let res;
    try {
      res = await api("add", { trackId: track.id, name: name || undefined, guest });
    } catch (e) {
      pending.delete(track.id);
      markAdded(button, false);
      if (!lostAccess(e)) toast(message(e.kind));
      return;
    }
    rememberBatch(res.batch, "track");
    toast(t.added(track.title));
  });
  return button;
}

// --- Démarrage -------------------------------------------------------------------

for (const node of document.querySelectorAll("[data-t]")) node.textContent = t[node.dataset.t];
$("query").placeholder = t.searchPlaceholder;

$("join-form").addEventListener("submit", (event) => {
  event.preventDefault();
  name = $("name").value.trim().slice(0, 24);
  if (name) localStorage.setItem(NAME_KEY, name);
  else localStorage.removeItem(NAME_KEY);
  join($("code").value);
});

$("search-form").addEventListener("submit", (event) => {
  event.preventDefault();
  $("query").blur();
  search($("query").value);
});
$("query").addEventListener("input", () => {
  $("clear").hidden = !$("query").value;
  scheduleSearch();
});

// Croix maison (#224) : celle de `type="search"` n'existe pas partout (Safari iOS, Firefox).
$("clear").setAttribute("aria-label", t.clear);
$("clear").addEventListener("click", () => {
  $("query").value = "";
  $("clear").hidden = true;
  search("");
  $("query").focus();
});

$("rename").addEventListener("click", () => showJoin());

for (const [id, which] of [
  ["tab-tracks", "tracks"],
  ["tab-albums", "albums"],
]) {
  $(id).addEventListener("click", () => {
    tab = which;
    showResults();
  });
}

// QR code de l'hôte : le code arrive après `#`, jamais envoyé au serveur. Retiré de
// l'adresse pour ne pas rester dans l'historique ni être partagé par erreur.
const fromQr = decodeURIComponent(location.hash.slice(1));
if (fromQr) {
  history.replaceState(null, "", location.pathname);
  code = fromQr;
}

if (code && (name || !fromQr)) join(code);
else showJoin();
