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
