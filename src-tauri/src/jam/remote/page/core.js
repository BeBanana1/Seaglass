// --- Shared helpers ------------------------------------------------------------

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
