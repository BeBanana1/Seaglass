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
    searching: "Recherche…",
    upNext: "À suivre",
    queueEmpty: "Rien dans la file pour l'instant.",
    nowPlaying: "En lecture",
    paused: "En pause",
    nothing: "Rien en lecture",
    noResults: "Aucun titre trouvé.",
    add: "Ajouter à la file",
    added: (t) => `« ${t} » ajouté à la file`,
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
  },
  en: {
    lead: "Add tracks to the host's queue.",
    nameLabel: "Your first name (shown next to your tracks)",
    codeLabel: "Jam code",
    join: "Join",
    searchPlaceholder: "Search for a track or artist…",
    searching: "Searching…",
    upNext: "Up next",
    queueEmpty: "Nothing in the queue yet.",
    nowPlaying: "Now playing",
    paused: "Paused",
    nothing: "Nothing playing",
    noResults: "No tracks found.",
    add: "Add to queue",
    added: (t) => `“${t}” added to the queue`,
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
  },
};
const lang = (navigator.language || "en").toLowerCase().startsWith("fr") ? "fr" : "en";
const t = TEXT[lang];
document.documentElement.lang = lang;

const $ = (id) => document.getElementById(id);
const CODE_KEY = "seaglass-jam-code";
const NAME_KEY = "seaglass-jam-name";
const SEARCH_DELAY = 400;

let code = localStorage.getItem(CODE_KEY) || "";
let name = localStorage.getItem(NAME_KEY) || "";
/** Flux de l'état poussé par l'hôte. */
let events = null;
/** Titres ajoutés depuis cette page, montrés tout de suite sans attendre l'hôte. */
const pending = new Set();

class ApiError extends Error {
  constructor(kind) {
    super(kind);
    this.kind = kind;
  }
}

async function api(path, body) {
  let res;
  try {
    res = await fetch(`/api/${path}`, {
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
    }[kind] || t.errBackend
  );
}

/** Code refusé ou adresse bloquée : retour à l'écran pour rejoindre. */
function lostAccess(e) {
  if (e.kind !== "code" && e.kind !== "blocked") return false;
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
  } else {
    now.append(el("div", "muted", t.nothing));
  }
  // L'hôte a pris en compte les ajouts en attente qu'il renvoie.
  for (const track of state.queue) pending.delete(track.id);
  $("queue").replaceChildren(...state.queue.map((track) => trackRow(track)));
  $("queue-empty").hidden = state.queue.length > 0;
}

function openEvents() {
  closeEvents();
  events = new EventSource(`/api/events?code=${encodeURIComponent(code)}`);
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
    results.replaceChildren();
    return;
  }
  status.textContent = t.searching;
  status.hidden = false;
  results.classList.add("stale");
  let tracks;
  try {
    tracks = await api("search", { query });
  } catch (e) {
    if (seq !== searchSeq) return;
    status.hidden = true;
    results.classList.remove("stale");
    if (!lostAccess(e)) toast(message(e.kind));
    return;
  }
  // Une recherche plus récente est partie entre-temps.
  if (seq !== searchSeq) return;
  status.hidden = tracks.length > 0;
  status.textContent = tracks.length > 0 ? "" : t.noResults;
  results.classList.remove("stale");
  results.replaceChildren(...tracks.map((track) => trackRow(track, addButton(track))));
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
    try {
      await api("add", { trackId: track.id, name: name || undefined });
    } catch (e) {
      pending.delete(track.id);
      markAdded(button, false);
      if (!lostAccess(e)) toast(message(e.kind));
      return;
    }
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
$("query").addEventListener("input", scheduleSearch);

$("rename").addEventListener("click", () => showJoin());

// QR code de l'hôte : le code arrive après `#`, jamais envoyé au serveur. Retiré de
// l'adresse pour ne pas rester dans l'historique ni être partagé par erreur.
const fromQr = decodeURIComponent(location.hash.slice(1));
if (fromQr) {
  history.replaceState(null, "", location.pathname);
  code = fromQr;
}

if (code && (name || !fromQr)) join(code);
else showJoin();
