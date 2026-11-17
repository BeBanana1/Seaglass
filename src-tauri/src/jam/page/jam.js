// Page des invités de la Jam (#25). Aucune donnée distante n'entre dans le HTML :
// tout passe par textContent.
"use strict";

const TEXT = {
  fr: {
    lead: "Ajoutez des titres à la file de l'hôte.",
    nameLabel: "Votre prénom (affiché à côté de vos titres)",
    codeLabel: "Code de la Jam",
    join: "Rejoindre",
    searchPlaceholder: "Titre, artiste, ou lien collé…",
    appsLabel: "Chercher dans :",
    pasteHint: "Copiez le lien d'un titre ou d'un album dans votre app (Spotify, Deezer, YouTube Music, Apple Music, TIDAL) et collez-le ici.",
    noLinkResults: "Rien trouvé pour ce lien. Essayez avec le titre.",
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
    errBlocked: "Trop d'essais : réessayez dans un quart d'heure.",
    errSlow: "Doucement : réessayez dans une minute.",
    errNetwork: "La Jam ne répond pas. Elle est peut-être terminée.",
    errBackend: "L'hôte n'a pas pu répondre. Réessayez.",
    errUnknown: "Ce titre n'est plus disponible : relancez la recherche.",
    errOffline: "L'hôte a perdu le réseau. Il devrait revenir dans un instant.",
    errEnded: "Cette Jam est terminée.",
    errControls: "L'hôte a désactivé la pause pour les invités.",
    errEmpty: "Cet album n'a aucun titre disponible.",
    errBanned: "L'hôte vous a retiré de cette Jam.",
    errClosed: "L'hôte a suspendu les ajouts pour l'instant.",
    errFull: "Vous avez atteint la limite de titres en attente : attendez que les vôtres passent.",
    errDuplicate: "Ce titre est déjà dans la file.",
    errQueue: "La file de la Jam est pleine : attendez que des titres passent.",
    errAlbums: "L'hôte n'accepte pas les albums entiers.",
    errVolume: "L'hôte a désactivé le volume pour les invités.",
    errMuted: "L'hôte a coupé le son.",
    volume: "Volume",
    lyrics: "Paroles",
    lyricsClose: "Fermer les paroles",
    lyricsLoading: "Chargement des paroles…",
    lyricsNone: "Pas de paroles pour ce titre.",
    lyricsCredit: (p) => `Paroles fournies par ${p}`,
    closed: "Ajouts suspendus par l'hôte.",
    limit: (n) => `Jusqu'à ${n} titre${n > 1 ? "s" : ""} en attente par personne.`,
    // Blind test à plusieurs (#441).
    game: "Blind test",
    gameRound: (n, total) => `Manche ${n} / ${total}`,
    gameListen: "Écoutez… Le titre s'affichera à la révélation.",
    gamePlaceholder: "Titre, artiste, ou les deux",
    gameSend: "Valider",
    gamePass: "Passer",
    gameSent: "Réponse envoyée. On attend les autres…",
    gameRevealIn: (s) => `Révélation dans ${s} s`,
    gameNext: "Prochaine manche dans un instant…",
    gameFinished: "Partie terminée",
    gameRanking: "Classement",
    gameHost: "Hôte",
    gameGuest: (id) => `Invité ${id}`,
    gameYou: "vous",
    gamePoints: (n) => (n === 0 ? "0 point" : `+${n} point${n > 1 ? "s" : ""}`),
    gameScore: (n) => `${n} pt${n > 1 ? "s" : ""}`,
    gameBoth: "Titre et artiste !",
    gameTitleOnly: "Bon titre",
    gameArtistOnly: "Bon artiste",
    gameMissed: "Raté",
    gameNoAnswer: "Pas de réponse",
    gameFastest: "le plus rapide",
    gameNoName: "Donnez votre prénom pour apparaître au classement sous votre nom.",
    gameNotice: "Blind test en cours : les ajouts reprennent après la partie.",
    errRound: "Trop tard : la manche est terminée.",
    errAnswered: "Vous avez déjà répondu à cette manche.",
    errPlayers: "La partie est complète.",
    errGame: "Blind test en cours : les ajouts reprennent après la partie.",
  },
  en: {
    lead: "Add tracks to the host's queue.",
    nameLabel: "Your first name (shown next to your tracks)",
    codeLabel: "Jam code",
    join: "Join",
    searchPlaceholder: "Track, artist, or pasted link…",
    appsLabel: "Search in:",
    pasteHint: "Copy the link of a track or album in your app (Spotify, Deezer, YouTube Music, Apple Music, TIDAL) and paste it here.",
    noLinkResults: "Nothing found for this link. Try the title instead.",
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
    errBlocked: "Too many attempts: try again in a quarter of an hour.",
    errSlow: "Slow down: try again in a minute.",
    errNetwork: "The Jam isn't responding. It may have ended.",
    errBackend: "The host couldn't answer. Try again.",
    errUnknown: "This track is no longer available: search again.",
    errOffline: "The host lost their connection. They should be back shortly.",
    errEnded: "This Jam has ended.",
    errControls: "The host has turned off pausing for guests.",
    errEmpty: "This album has no available tracks.",
    errBanned: "The host removed you from this Jam.",
    errClosed: "The host has paused adding for now.",
    errFull: "You reached the limit of waiting tracks: wait for yours to play.",
    errDuplicate: "This track is already in the queue.",
    errQueue: "The Jam's queue is full: wait for some tracks to play.",
    errAlbums: "The host doesn't accept whole albums.",
    errVolume: "The host has turned off the volume for guests.",
    errMuted: "The host muted the sound.",
    volume: "Volume",
    lyrics: "Lyrics",
    lyricsClose: "Close the lyrics",
    lyricsLoading: "Loading the lyrics…",
    lyricsNone: "No lyrics for this track.",
    lyricsCredit: (p) => `Lyrics provided by ${p}`,
    closed: "Adding is paused by the host.",
    limit: (n) => `Up to ${n} waiting track${n > 1 ? "s" : ""} per person.`,
    game: "Blind test",
    gameRound: (n, total) => `Round ${n} / ${total}`,
    gameListen: "Listen… The title shows up at the reveal.",
    gamePlaceholder: "Title, artist, or both",
    gameSend: "Submit",
    gamePass: "Pass",
    gameSent: "Answer sent. Waiting for the others…",
    gameRevealIn: (s) => `Reveal in ${s} s`,
    gameNext: "Next round in a moment…",
    gameFinished: "Game over",
    gameRanking: "Ranking",
    gameHost: "Host",
    gameGuest: (id) => `Guest ${id}`,
    gameYou: "you",
    gamePoints: (n) => (n === 0 ? "0 points" : `+${n} point${n > 1 ? "s" : ""}`),
    gameScore: (n) => `${n} pt${n > 1 ? "s" : ""}`,
    gameBoth: "Title and artist!",
    gameTitleOnly: "Right title",
    gameArtistOnly: "Right artist",
    gameMissed: "Missed",
    gameNoAnswer: "No answer",
    gameFastest: "fastest",
    gameNoName: "Give your first name to show up in the ranking under your name.",
    gameNotice: "Blind test in progress: adding resumes after the game.",
    errRound: "Too late: the round is over.",
    errAnswered: "You already answered this round.",
    errPlayers: "The game is full.",
    errGame: "Blind test in progress: adding resumes after the game.",
  },
};
const lang = (navigator.language || "en").toLowerCase().startsWith("fr") ? "fr" : "en";
const t = TEXT[lang];
document.documentElement.lang = lang;

const $ = (id) => document.getElementById(id);
/** Code rangé par Jam (#256) : sur le relais, toutes les Jams partagent l'origine, et un
 * ancien code gâcherait un essai sur la nouvelle. Le chemin est `/j/<id>/` (relais) ou `/`
 * (réseau local). L'ancienne clé commune est retirée. */
const CODE_KEY = `seaglass-jam-code:${location.pathname}`;
localStorage.removeItem("seaglass-jam-code");
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
    // Le jeton part avec chaque requête : l'hôte peut exclure un invité (#288).
    res = await fetch(`api/${path}`, {
      method: "POST",
      headers: { "Content-Type": "application/json", "X-Jam-Code": code },
      body: JSON.stringify({ ...body, guest }),
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
      banned: t.errBanned,
      closed: t.errClosed,
      full: t.errFull,
      duplicate: t.errDuplicate,
      queue: t.errQueue,
      albums: t.errAlbums,
      volume: t.errVolume,
      muted: t.errMuted,
      round: t.errRound,
      answered: t.errAnswered,
      players: t.errPlayers,
      game: t.errGame,
    }[kind] || t.errBackend
  );
}

/** Code refusé, adresse bloquée, invité exclu (#288) ou Jam terminée (relais) : retour à
 * l'écran pour rejoindre. */
function lostAccess(e) {
  if (!["code", "blocked", "ended", "banned"].includes(e.kind)) return false;
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
  loadPlayer();
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
    // Paroles (#448) : masqué pendant un blind test, ou quand l'hôte sait qu'il n'y en a pas.
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
  followLyrics(state);
  // Règles de l'hôte (#288) : ajouts suspendus, limite par invité, albums.
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

/** Volume (#449), si l'hôte le permet : le curseur s'arrête au plafond de l'hôte, et n'est
 * pas déplacé par l'état poussé pendant que l'invité le tient. */
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
  // L'état réel revient par le flux ; en cas de refus, le curseur reprend sa place.
  if (lastState) renderVolume(lastState.volume);
}

// --- Paroles (#448) ----------------------------------------------------------------
// Paroles du titre en cours seulement, demandées à l'hôte. Synchronisées : la ligne en
// cours suit la position, déduite du point de départ du titre (`startedAt`, heure de
// l'hôte, corrigée de l'écart d'horloge reçu avec les paroles) ou de la position en pause.

/** Titre dont les paroles sont affichées (`null` panneau fermé), lignes horodatées. */
let lyricsTrack = null;
let lyricsLines = [];
/** Heure de l'hôte moins celle de ce téléphone, en ms. */
let clockOffset = 0;
let lyricsTimer = null;
let currentLine = -1;
/** L'invité fait défiler les paroles : on arrête de suivre un moment. */
let lyricsHeldUntil = 0;

function openLyrics() {
  $("lyrics").hidden = false;
  document.body.classList.add("locked");
  loadLyrics();
  $("lyrics-close").focus();
}

function closeLyrics() {
  $("lyrics").hidden = true;
  document.body.classList.remove("locked");
  lyricsTrack = null;
  lyricsLines = [];
  clearInterval(lyricsTimer);
}

async function loadLyrics() {
  const now = lastState?.now;
  lyricsTrack = now?.id ?? null;
  lyricsLines = [];
  currentLine = -1;
  clearInterval(lyricsTimer);
  $("lyrics-title").textContent = now?.title ?? "";
  $("lyrics-sub").textContent = now?.artists ?? "";
  $("lyrics-credit").hidden = true;
  const body = $("lyrics-body");
  body.className = "lyrics-body plain";
  body.dir = "auto";
  body.replaceChildren(el("p", "muted", t.lyricsLoading));
  let res;
  try {
    res = await api("lyrics");
  } catch (e) {
    if (lostAccess(e)) return;
    toast(message(e.kind));
    closeLyrics();
    return;
  }
  // Fermé, ou le titre a changé pendant la requête : la suite s'en charge.
  if ($("lyrics").hidden || res.trackId !== lyricsTrack) return;
  if (typeof res.now === "number") clockOffset = res.now - Date.now();
  const lyrics = res.lyrics;
  if (!lyrics) {
    body.replaceChildren(el("p", "", t.lyricsNone));
    return;
  }
  body.dir = lyrics.rtl ? "rtl" : "auto";
  if (lyrics.provider) {
    $("lyrics-credit").textContent = t.lyricsCredit(lyrics.provider);
    $("lyrics-credit").hidden = false;
  }
  if (lyrics.text.kind === "synced") {
    lyricsLines = lyrics.text.lines;
    body.className = "lyrics-body";
    body.replaceChildren(...lyricsLines.map((l) => el("p", l.text ? "" : "gap", l.text || "♪")));
    syncLyrics();
    lyricsTimer = setInterval(syncLyrics, 250);
  } else {
    body.replaceChildren(...lyrics.text.lines.map((l) => el("p", l ? "" : "gap", l)));
  }
}

/** Position du titre en cours en ms d'après l'état poussé, `null` inconnue. */
function playbackPosition() {
  const s = lastState;
  if (!s) return null;
  if (typeof s.startedAt === "number") return Date.now() + clockOffset - s.startedAt;
  if (typeof s.position === "number") return s.position;
  return null;
}

function syncLyrics() {
  const position = playbackPosition();
  if (position === null || lyricsLines.length === 0) return;
  let index = -1;
  for (let i = 0; i < lyricsLines.length && lyricsLines[i].timeMs <= position; i++) index = i;
  if (index === currentLine) return;
  const lines = $("lyrics-body").children;
  lines[currentLine]?.classList.remove("current");
  currentLine = index;
  const line = lines[index];
  if (!line) return;
  line.classList.add("current");
  if (Date.now() >= lyricsHeldUntil) line.scrollIntoView({ block: "center", behavior: "smooth" });
}

/** État poussé : nouveau titre (paroles rechargées), blind test ou plus rien (fermé). */
function followLyrics(state) {
  if ($("lyrics").hidden) return;
  if (!state.now || state.blind) {
    closeLyrics();
    return;
  }
  if (state.now.id !== lyricsTrack) loadLyrics();
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
  const { tracks } = lastHits;
  // L'hôte peut refuser les albums entiers (#288) : l'onglet disparaît.
  const albums = lastState?.albums === false ? [] : lastHits.albums;
  const status = $("search-status");
  const any = tracks.length + albums.length > 0;
  status.hidden = any;
  status.textContent = any ? "" : lastHits.link ? t.noLinkResults : t.noResults;
  // Onglets seulement s'il y a des titres et des albums (un lien n'en donne qu'un).
  $("tabs").hidden = albums.length === 0 || tracks.length === 0;
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
  if (!$("game").hidden) {
    status.hidden = true;
    $("tabs").hidden = true;
    return;
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


// --- Blind test à plusieurs (#441) ------------------------------------------------
// La partie est tenue par l'hôte : la page affiche la manche, envoie une réponse, puis le
// résultat et le classement. Le titre n'arrive qu'à la révélation.

/** Identifiant de joueur de ce navigateur dans la partie, rendu par la première réponse. */
const PLAYER_KEY = `seaglass-jam-player:${location.pathname}`;
let player = null;
function loadPlayer() {
  try {
    const saved = JSON.parse(localStorage.getItem(PLAYER_KEY) || "null");
    player = saved && saved.code === code ? saved.id : null;
  } catch {
    player = null;
  }
}
/** Manche à laquelle cette page vient de répondre, avant que l'hôte le confirme. */
let answeredRound = 0;
/** Clé de ce qui est affiché dans `#game-play` : reconstruit seulement quand elle change
 *  (sinon la saisie en cours serait effacée à chaque état poussé). */
let playKey = "";
let revealTimer = null;

function playerName(p) {
  if (p.host) return t.gameHost;
  return p.name || t.gameGuest(p.id);
}

function verdict(r) {
  if (!r.answered) return t.gameNoAnswer;
  if (r.title && r.artist) return t.gameBoth;
  if (r.title) return t.gameTitleOnly;
  if (r.artist) return t.gameArtistOnly;
  return t.gameMissed;
}

function renderGame(game) {
  const playing = Boolean(game);
  $("game").hidden = !playing;
  for (const id of ["search-form", "search-hint", "search-status", "tabs", "results"]) {
    if (playing) $(id).hidden = true;
  }
  if (!playing) {
    $("search-form").hidden = false;
    $("search-hint").hidden = false;
    $("results").hidden = false;
    if (playKey) {
      playKey = "";
      showResults();
    }
    clearInterval(revealTimer);
    return;
  }
  $("game-title").textContent = game.title ? `${t.game} · ${game.title}` : t.game;
  $("game-round").textContent = t.gameRound(game.round, game.rounds);
  const me = game.players.find((p) => p.id === player);
  const answered = answeredRound === game.round || Boolean(me?.answered);
  const key = `${game.round}:${game.phase}:${answered}`;
  if (key !== playKey) {
    playKey = key;
    $("game-play").replaceChildren(...gamePlay(game, answered));
  }
  showCountdown(game);
  renderBoard(game);
}

function gamePlay(game, answered) {
  if (game.phase === "finished") return [el("p", "game-status", t.gameFinished)];
  if (game.phase === "revealed" && game.last) return reveal(game.last);
  const nodes = [el("p", "game-status", answered ? t.gameSent : t.gameListen)];
  if (answered) return nodes;
  if (!name) nodes.push(el("p", "hint", t.gameNoName));
  if (game.mode === "choices") {
    const list = el("div", "choices");
    game.choices.forEach((choice, index) => {
      const button = el("button", "choice");
      button.type = "button";
      button.append(el("span", "title", choice.title), el("span", "sub", choice.artist));
      button.addEventListener("click", () => sendAnswer(game.round, { choice: index }, list));
      list.append(button);
    });
    nodes.push(list);
  } else {
    const form = el("form", "game-answer");
    form.autocomplete = "off";
    const input = el("input");
    input.maxLength = 200;
    input.placeholder = t.gamePlaceholder;
    input.setAttribute("aria-label", t.gamePlaceholder);
    input.enterKeyHint = "send";
    const send = el("button", "", t.gameSend);
    send.type = "submit";
    form.append(input, send);
    form.addEventListener("submit", (event) => {
      event.preventDefault();
      if (input.value.trim()) sendAnswer(game.round, { text: input.value }, form);
    });
    nodes.push(form);
    requestAnimationFrame(() => input.focus());
  }
  const pass = el("button", "link pass", t.gamePass);
  pass.type = "button";
  pass.addEventListener("click", () => sendAnswer(game.round, {}, pass));
  nodes.push(pass);
  return nodes;
}

async function sendAnswer(round, answer, block) {
  for (const node of [block, ...block.querySelectorAll("button, input")]) node.disabled = true;
  let res;
  try {
    res = await api("blind-answer", { round, name: name || undefined, ...answer });
  } catch (e) {
    for (const node of [block, ...block.querySelectorAll("button, input")]) node.disabled = false;
    if (lostAccess(e)) return;
    toast(message(e.kind));
    if (e.kind !== "round" && e.kind !== "answered") return;
  }
  if (res && typeof res.player === "number") {
    player = res.player;
    localStorage.setItem(PLAYER_KEY, JSON.stringify({ code, id: player }));
  }
  answeredRound = round;
  if (lastState) render(lastState);
}

function reveal(last) {
  const card = el("div", "game-reveal");
  card.append(cover(last.track.cover));
  const meta = el("div", "meta");
  meta.append(el("div", "title", last.track.title), el("div", "sub", last.track.artists));
  card.append(meta);
  const nodes = [card];
  const mine = last.players.find((p) => p.id === player);
  if (mine) {
    const line = el("p", `game-verdict${mine.points > 0 ? " good" : ""}`);
    line.append(el("strong", "", verdict(mine)), ` ${t.gamePoints(mine.points)}`);
    if (mine.fastest) line.append(` · ${t.gameFastest}`);
    nodes.push(line);
  }
  nodes.push(el("p", "muted", t.gameNext));
  return nodes;
}

function showCountdown(game) {
  clearInterval(revealTimer);
  const node = $("game-countdown");
  const tick = () => {
    const left = game.revealAt ? Math.ceil((game.revealAt - Date.now()) / 1000) : 0;
    node.hidden = game.phase !== "guess" || left <= 0;
    node.textContent = left > 0 ? t.gameRevealIn(left) : "";
  };
  tick();
  if (game.revealAt && game.phase === "guess") revealTimer = setInterval(tick, 1000);
}

function renderBoard(game) {
  const last = game.phase === "guess" ? null : game.last;
  const rows = game.players.map((p, i) => {
    const li = el("li", `player${p.id === player ? " me" : ""}`);
    li.append(el("span", "rank", String(i + 1)));
    const who = el("span", "player-name", playerName(p));
    if (p.id === player) who.append(el("span", "you", ` (${t.gameYou})`));
    li.append(who);
    const round = last?.players.find((r) => r.id === p.id);
    if (round) li.append(el("span", `gain${round.points > 0 ? " good" : ""}`, t.gamePoints(round.points)));
    else if (game.phase === "guess" && p.answered) li.append(el("span", "gain good", "✓"));
    li.append(el("span", "score", t.gameScore(p.score)));
    return li;
  });
  $("game-board").replaceChildren(...rows);
  $("game-board-title").hidden = rows.length === 0;
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

$("lyrics-close").setAttribute("aria-label", t.lyricsClose);
$("lyrics-close").addEventListener("click", closeLyrics);
document.addEventListener("keydown", (event) => {
  if (event.key === "Escape" && !$("lyrics").hidden) closeLyrics();
});
for (const event of ["wheel", "touchmove"]) {
  $("lyrics-body").addEventListener(event, () => (lyricsHeldUntil = Date.now() + 4000), {
    passive: true,
  });
}

// Un envoi par geste (`change`), pas à chaque pas du curseur (`input`).
$("volume").setAttribute("aria-label", t.volume);
$("volume").addEventListener("input", () => {
  volumeDragging = true;
  $("volume-value").textContent = $("volume").value;
});
$("volume").addEventListener("change", sendVolume);
// Geste annulé (le navigateur reprend le doigt pour défiler) : pas de `change`, le curseur
// doit pourtant suivre de nouveau l'état poussé.
for (const event of ["pointercancel", "blur"]) {
  $("volume").addEventListener(event, () => {
    if (!volumeDragging) return;
    volumeDragging = false;
    if (lastState) renderVolume(lastState.volume);
  });
}

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
// Un fragment mal formé (`%` isolé) ferait lever `decodeURIComponent` : ignoré (#262).
let fromQr = "";
try {
  fromQr = decodeURIComponent(location.hash.slice(1));
} catch {
  history.replaceState(null, "", location.pathname);
}
if (fromQr) {
  history.replaceState(null, "", location.pathname);
  code = fromQr;
}

if (code && (name || !fromQr)) join(code);
else showJoin();
