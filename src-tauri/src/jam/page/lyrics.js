// --- Lyrics (#448) ----------------------------------------------------------------
// Lyrics of the current track only, requested from the host. Synchronized: the current
// line follows the position, derived from the track's start point (`startedAt`, host time,
// corrected by the clock offset received with the lyrics) or from the paused position.

/** Track whose lyrics are shown (`null` panel closed), timestamped lines. */
let lyricsTrack = null;
let lyricsLines = [];
/** Host time minus this phone's time, in ms. */
let clockOffset = 0;
let lyricsTimer = null;
let currentLine = -1;
/** The guest is scrolling the lyrics: stop following for a while. */
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
  // Closed, or the track changed during the request: what follows takes care of it.
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

/** Position of the current track in ms from the pushed state, `null` unknown. */
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

/** Pushed state: new track (lyrics reloaded), blind test or nothing (closed). */
function followLyrics(state) {
  if ($("lyrics").hidden) return;
  if (!state.now || state.blind) {
    closeLyrics();
    return;
  }
  if (state.now.id !== lyricsTrack) loadLyrics();
}
