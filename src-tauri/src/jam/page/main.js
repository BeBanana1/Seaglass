// --- Startup -------------------------------------------------------------------

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

// Custom cross (#224): the one of `type="search"` does not exist everywhere (iOS Safari,
// Firefox).
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

// One send per gesture (`change`), not at every step of the slider (`input`).
$("volume").setAttribute("aria-label", t.volume);
$("volume").addEventListener("input", () => {
  volumeDragging = true;
  $("volume-value").textContent = $("volume").value;
});
$("volume").addEventListener("change", sendVolume);
$("skip").addEventListener("click", voteSkip);
// Gesture cancelled (the browser takes the finger back to scroll): no `change`, yet the
// slider must follow the pushed state again.
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

// Host's QR code: the code comes after `#`, never sent to the server. Removed from the
// address so it does not stay in the history or get shared by mistake.
// A malformed fragment (lone `%`) would make `decodeURIComponent` throw: ignored (#262).
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
