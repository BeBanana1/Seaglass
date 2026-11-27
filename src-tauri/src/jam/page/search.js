// --- Search and add ---------------------------------------------------------

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
  // A more recent search went out in the meantime.
  if (seq !== searchSeq) return;
  // A host from before #223 only returns tracks.
  lastHits = Array.isArray(hits) ? { tracks: hits, albums: [] } : hits;
  results.classList.remove("stale");
  showResults();
}

let lastHits = { tracks: [], albums: [] };
let tab = "tracks";

function showResults() {
  const { tracks } = lastHits;
  // The host can refuse whole albums (#288): the tab disappears.
  const albums = lastState?.albums === false ? [] : lastHits.albums;
  const status = $("search-status");
  const any = tracks.length + albums.length > 0;
  status.hidden = any;
  status.textContent = any ? "" : lastHits.link ? t.noLinkResults : t.noResults;
  // Tabs only if there are both tracks and albums (a link only gives one).
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
    // Shown right away, undone if the host refuses.
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
