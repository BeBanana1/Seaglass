// Page des invités de la Jam (#25). Aucune donnée distante n'entre dans le HTML :
// tout passe par textContent.
"use strict";

const TEXT = {
  fr: {
    lead: "Ajoutez des titres à la file de l'hôte.",
    codeLabel: "Code de la Jam",
    join: "Rejoindre",
    search: "Chercher",
    searchPlaceholder: "Titre, artiste…",
    upNext: "À suivre",
    queueEmpty: "Rien dans la file pour l'instant.",
    nowPlaying: "En lecture",
    paused: "En pause",
    nothing: "Rien en lecture",
    noResults: "Aucun titre trouvé.",
    add: "Ajouter à la file",
    added: (t) => `« ${t} » ajouté à la file`,
    errCode: "Code incorrect.",
    errBlocked: "Trop d'essais : demandez à l'hôte de relancer la Jam.",
    errSlow: "Doucement : réessayez dans une minute.",
    errNetwork: "La Jam ne répond pas. Elle est peut-être terminée.",
    errBackend: "L'hôte n'a pas pu répondre. Réessayez.",
    errUnknown: "Ce titre n'est plus disponible : relancez la recherche.",
  },
  en: {
    lead: "Add tracks to the host's queue.",
    codeLabel: "Jam code",
    join: "Join",
    search: "Search",
    searchPlaceholder: "Track, artist…",
    upNext: "Up next",
    queueEmpty: "Nothing in the queue yet.",
    nowPlaying: "Now playing",
    paused: "Paused",
    nothing: "Nothing playing",
    noResults: "No tracks found.",
    add: "Add to queue",
    added: (t) => `“${t}” added to the queue`,
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
let code = localStorage.getItem(CODE_KEY) || "";
let poll = null;

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
  li.append(meta);
  if (action) li.append(action);
  return li;
}

// --- Rejoindre ---------------------------------------------------------------

function showJoin(error) {
  stopPolling();
  $("jam").hidden = true;
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
    if (e.kind === "code" || e.kind === "blocked") localStorage.removeItem(CODE_KEY);
    showJoin(message(e.kind));
    return;
  }
  localStorage.setItem(CODE_KEY, code);
  $("join").hidden = true;
  $("jam").hidden = false;
  refresh();
  startPolling();
}

// --- État de la Jam -------------------------------------------------------------

async function refresh() {
  let state;
  try {
    state = await api("state");
  } catch (e) {
    if (e.kind === "code" || e.kind === "blocked") return showJoin(message(e.kind));
    return;
  }
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
  $("queue").replaceChildren(...state.queue.map((track) => trackRow(track)));
  $("queue-empty").hidden = state.queue.length > 0;
}

function startPolling() {
  stopPolling();
  poll = setInterval(() => {
    if (document.visibilityState === "visible") refresh();
  }, 5000);
}

function stopPolling() {
  clearInterval(poll);
  poll = null;
}

document.addEventListener("visibilitychange", () => {
  if (document.visibilityState === "visible" && poll) refresh();
});

// --- Recherche et ajout ---------------------------------------------------------

async function search(query) {
  const results = $("results");
  if (!query.trim()) {
    results.replaceChildren();
    return;
  }
  let tracks;
  try {
    tracks = await api("search", { query });
  } catch (e) {
    if (e.kind === "code" || e.kind === "blocked") return showJoin(message(e.kind));
    toast(message(e.kind));
    return;
  }
  if (tracks.length === 0) {
    results.replaceChildren(el("li", "muted", t.noResults));
    return;
  }
  results.replaceChildren(...tracks.map((track) => trackRow(track, addButton(track))));
}

function addButton(track) {
  const button = el("button", "add", "+");
  button.type = "button";
  button.setAttribute("aria-label", `${t.add} : ${track.title}`);
  button.addEventListener("click", async () => {
    button.disabled = true;
    try {
      await api("add", { trackId: track.id });
    } catch (e) {
      button.disabled = false;
      if (e.kind === "code" || e.kind === "blocked") return showJoin(message(e.kind));
      toast(message(e.kind));
      return;
    }
    button.textContent = "✓";
    button.classList.add("done");
    toast(t.added(track.title));
    refresh();
  });
  return button;
}

// --- Démarrage -------------------------------------------------------------------

for (const node of document.querySelectorAll("[data-t]")) node.textContent = t[node.dataset.t];
$("query").placeholder = t.searchPlaceholder;

$("join-form").addEventListener("submit", (event) => {
  event.preventDefault();
  join($("code").value);
});

$("search-form").addEventListener("submit", (event) => {
  event.preventDefault();
  $("query").blur();
  search($("query").value);
});

if (code) join(code);
else showJoin();
