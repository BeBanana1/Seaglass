import {
  emit,
  fx,
  libraryMixes,
  longCount,
  longTracks,
  mixPage,
  params,
  type Handlers,
} from "./core";
import type {
  AlbumSummary,
  ArtistSummary,
  BlockTarget,
  Blocked,
  LaterItem,
  LibraryList,
  LikedIds,
  LikeTarget,
} from "../ipc";

// Écritures (#4, #81) : favoris et playlists de l'utilisateur, gardés en mémoire.
// `?truncated=1` : chaque catégorie se présente comme coupée au plafond (#91), avec un
// total annoncé de 12 345 (`?truncated=nototal` : total inconnu).
function libraryList<T>(items: T[]): LibraryList<T> {
  const mode = params.get("truncated");
  if (!mode) return { items, truncated: false, total: items.length };
  return { items, truncated: true, total: mode === "nototal" ? null : 12345 };
}

const liked = new Set<number>(fx.library.tracks.map((t) => t.id));
const favAlbums = [...(fx.library.albums as AlbumSummary[])];
const favArtists = [...(fx.library.artists as ArtistSummary[])];
export const libPlaylists = [...fx.library.playlists];

function likedIds(): LikedIds {
  return {
    tracks: [...liked],
    albums: favAlbums.map((a) => a.id),
    artists: favArtists.map((a) => a.id),
    playlists: libPlaylists.filter((p) => !p.mine).map((p) => p.uuid),
    mixes: libraryMixes.map((m) => m.id),
  };
}

/** Ajoute en tête (comme le tri par date du service) ou retire un favori de la liste. */
function toggleIn<T>(list: T[], known: T[], match: (x: T) => boolean, on: boolean) {
  const i = list.findIndex(match);
  if (i >= 0 && !on) list.splice(i, 1);
  const found = known.find(match);
  if (i < 0 && on && found) list.unshift(found);
}

function setLiked(target: LikeTarget, on: boolean) {
  switch (target.kind) {
    case "track":
      if (on) liked.add(Number(target.id));
      else liked.delete(Number(target.id));
      break;
    case "album": {
      const known = [
        fx.album.album,
        ...fx.search.albums,
        ...fx.artist.albums,
        ...fx.artist.singles,
      ];
      toggleIn(favAlbums, known, (a) => a.id === Number(target.id), on);
      break;
    }
    case "artist":
      toggleIn(
        favArtists,
        [fx.artist.artist, ...fx.search.artists],
        (a) => a.id === Number(target.id),
        on,
      );
      break;
    case "playlist":
      toggleIn(
        libPlaylists,
        [fx.playlist.playlist, ...fx.search.playlists],
        (p) => p.uuid === target.uuid,
        on,
      );
      break;
    case "mix": {
      // Ajouté en fin de liste, comme le service (#204).
      const i = libraryMixes.findIndex((m) => m.id === target.id);
      if (i >= 0 && !on) libraryMixes.splice(i, 1);
      if (i < 0 && on)
        libraryMixes.push(
          mixPage.mix.id === target.id ? mixPage.mix : { ...mixPage.mix, id: target.id },
        );
      break;
    }
  }
  emit("likes", likedIds());
}
// « Plus tard » (#196) : deux titres et un album de démonstration, plus récent en tête.
const laterItems: LaterItem[] = [
  ...fx.library.tracks.slice(0, 2).map((t) => ({ kind: "track" as const, ...t })),
  { kind: "album" as const, ...fx.album.album },
];

function laterIds(): LikedIds {
  const ids: LikedIds = { tracks: [], albums: [], artists: [], playlists: [], mixes: [] };
  for (const item of laterItems) {
    if (item.kind === "track") ids.tracks.push(item.id);
    else if (item.kind === "album") ids.albums.push(item.id);
    else if (item.kind === "artist") ids.artists.push(item.id);
    else ids.playlists.push(item.uuid);
  }
  return ids;
}

function laterKey(item: LaterItem | LikeTarget): string {
  return item.kind === "playlist" ? `playlist:${item.uuid}` : `${item.kind}:${item.id}`;
}

function setSavedForLater(target: LikeTarget, on: boolean) {
  const i = laterItems.findIndex((item) => laterKey(item) === laterKey(target));
  if (i >= 0 && !on) laterItems.splice(i, 1);
  if (i < 0 && on) {
    const known: LaterItem[] = [
      ...fx.library.tracks.map((t) => ({ kind: "track" as const, ...t })),
      ...[fx.album.album, ...fx.search.albums].map((a) => ({ kind: "album" as const, ...a })),
      ...[fx.artist.artist, ...fx.search.artists].map((a) => ({ kind: "artist" as const, ...a })),
      ...[fx.playlist.playlist, ...fx.search.playlists].map((p) => ({
        kind: "playlist" as const,
        ...p,
      })),
    ];
    const found = known.find((item) => laterKey(item) === laterKey(target));
    if (found) laterItems.unshift(found);
  }
  emit("later", laterIds());
}

// Blocages (#334) : un titre et un artiste de démonstration, plus récent en tête.
const blocked: Blocked = {
  tracks: fx.search.tracks.slice(-1).map((t) => ({ ...t })),
  artists: fx.search.artists.slice(-1).map((a) => ({ ...a })),
};

function setBlocked(target: BlockTarget, on: boolean) {
  if (target.kind === "track") {
    const id = Number(target.id);
    const known = [...fx.search.tracks, ...fx.album.tracks, ...fx.library.tracks];
    toggleIn(blocked.tracks, known, (t) => t.id === id, on);
  } else {
    const known = [fx.artist.artist, ...fx.search.artists];
    toggleIn(blocked.artists, known, (a) => a.id === Number(target.id), on);
  }
}

export const favoritesHandlers: Handlers = {
  likes: () => likedIds(),
  // Les arguments sont des proxys Svelte : copiés en JSON avant d'être gardés.
  set_liked: ({ target, liked: on }) => setLiked(JSON.parse(JSON.stringify(target)), Boolean(on)),
  set_tracks_liked: ({ trackIds, liked: on }) => {
    for (const id of trackIds as number[]) setLiked({ kind: "track", id }, Boolean(on));
  },
  blocked: () => structuredClone(blocked),
  set_blocked: ({ target, blocked: on }) =>
    setBlocked(JSON.parse(JSON.stringify(target)), Boolean(on)),
  later_ids: () => laterIds(),
  save_for_later: () => libraryList([...laterItems]),
  set_saved_for_later: ({ target, saved }) =>
    setSavedForLater(JSON.parse(JSON.stringify(target)), Boolean(saved)),
  library_mixes: () => libraryMixes,
  library_tracks: () => libraryList(longCount ? longTracks : fx.library.tracks),
  library_albums: () => libraryList([...favAlbums]),
  library_artists: () => libraryList([...favArtists]),
  library_playlists: () => libraryList([...libPlaylists]),
  // Copie « du lancement précédent » (#181) : la liste actuelle, comme si rien n'avait changé.
  cached_library: ({ tab }) =>
    (<Record<string, () => unknown>>{
      tracks: () => libraryList(longCount ? longTracks : fx.library.tracks),
      albums: () => libraryList([...favAlbums]),
      artists: () => libraryList([...favArtists]),
      playlists: () => libraryList([...libPlaylists]),
    })[String(tab)]?.() ?? null,
};
