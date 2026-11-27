// --- Shared helpers ------------------------------------------------------------

const $ = (id) => document.getElementById(id);
/** Code stored per Jam (#256): on the relay, all Jams share the origin, and an old code
 * would waste an attempt on the new one. The path is `/j/<id>/` (relay) or `/` (local
 * network). The old shared key is removed. */
const CODE_KEY = `seaglass-jam-code:${location.pathname}`;
localStorage.removeItem("seaglass-jam-code");
const NAME_KEY = "seaglass-jam-name";
const GUEST_KEY = "seaglass-jam-guest";
const MINE_KEY = "seaglass-jam-mine";
const SEARCH_DELAY = 400;

let code = localStorage.getItem(CODE_KEY) || "";
let name = localStorage.getItem(NAME_KEY) || "";
/** Stream of the state pushed by the host. */
let events = null;
/** Tracks added from this page, shown right away without waiting for the host. */
const pending = new Set();
const pendingAlbums = new Set();

/** Token of this browser (#223): the host only lets an addition be removed by its author.
 * `getRandomValues` also exists over plain HTTP (local network), `randomUUID` does not. */
const guest =
  localStorage.getItem(GUEST_KEY) ||
  (() => {
    const bytes = crypto.getRandomValues(new Uint8Array(16));
    const token = Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
    localStorage.setItem(GUEST_KEY, token);
    return token;
  })();

/** Batches added from this browser during the current Jam: `{ <batch>: "track" | "album" }`. */
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
    // Relative paths: the page is served at `/` (local network) or `/j/<id>/` (relay).
    // The token goes with every request: the host can exclude a guest (#288).
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
      skip: t.errSkip,
      changed: t.errChanged,
      round: t.errRound,
      answered: t.errAnswered,
      players: t.errPlayers,
      game: t.errGame,
    }[kind] || t.errBackend
  );
}

/** Code refused, address blocked, guest excluded (#288) or Jam ended (relay): back to the
 * join screen. */
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
