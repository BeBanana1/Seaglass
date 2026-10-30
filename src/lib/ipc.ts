// Wrappers typés des commandes et événements Tauri.
// Types dupliqués depuis Rust : tidal/models.rs, player/state.rs, mpv/events.rs.

import { invoke, listen, type UnlistenFn } from "./transport";

export type TrackId = number;

export interface ArtistRef {
  id: number;
  name: string;
}

export interface AlbumRef {
  id: number;
  title: string;
  coverUrl: string | null;
  /** Couleur dominante de la pochette (`#rrggbb`). */
  color: string | null;
}

export interface Track {
  id: TrackId;
  title: string;
  version: string | null;
  /** Secondes. */
  duration: number;
  artists: ArtistRef[];
  album: AlbumRef;
  explicit: boolean;
  hires: boolean;
}

/** Titre d'une liste (playlist ou favoris) avec sa date d'ajout à cette liste. */
export interface TrackEntry extends Track {
  /** Millisecondes depuis l'epoch Unix (UTC) ; `null` si l'API ne la donne pas. */
  addedAt: number | null;
}

export type AudioQuality = "LOW" | "HIGH" | "LOSSLESS" | "HI_RES_LOSSLESS";

export interface StreamFormat {
  quality: AudioQuality;
  codec: string;
  bitDepth: number | null;
  sampleRate: number | null;
}

export interface AudioParams {
  format: string;
  samplerate: number;
  channelCount: number;
}

export type Repeat = "off" | "all" | "one";

export interface PlayerState {
  track: Track | null;
  stream: StreamFormat | null;
  output: AudioParams | null;
  outputDriver: string | null;
  loading: boolean;
  paused: boolean;
  buffering: boolean;
  idle: boolean;
  timePos: number;
  duration: number;
  shuffle: boolean;
  repeat: Repeat;
  hasPrevious: boolean;
  hasNext: boolean;
  volume: Volume;
}

/** Volume logiciel de mpv : 100 non coupé = gain unitaire (seul réglage bit-perfect). */
export interface Volume {
  /** 0 à 100. */
  level: number;
  muted: boolean;
}

export interface QueueView {
  manual: Track[];
  upcoming: Track[];
}

export interface AlbumSummary {
  id: number;
  title: string;
  artists: ArtistRef[];
  coverUrl: string | null;
  color: string | null;
  /** `AAAA-MM-JJ`. */
  releaseDate: string | null;
  numberOfTracks: number | null;
  /** `ALBUM`, `EP` ou `SINGLE`. */
  albumType: string | null;
  explicit: boolean;
  hires: boolean;
}

export interface ArtistSummary {
  id: number;
  name: string;
  pictureUrl: string | null;
}

export interface PlaylistSummary {
  uuid: string;
  title: string;
  description: string | null;
  imageUrl: string | null;
  numberOfTracks: number | null;
  duration: number | null;
  editorial: boolean;
  mine: boolean;
}

export type MediaItem =
  | ({ kind: "album" } & AlbumSummary)
  | ({ kind: "artist" } & ArtistSummary)
  | ({ kind: "playlist" } & PlaylistSummary);

export interface HomeRow {
  /** Identifiant stable de la section (`moduleId` de l'API), indépendant du titre. */
  key: string;
  title: string;
  items: MediaItem[];
}

/** Ligne de l'accueil dans l'ordre choisi ; les masquées ne sont montrées qu'en personnalisation. */
export interface HomeSection extends HomeRow {
  hidden: boolean;
}

/** Disposition de l'accueil enregistrée (clés de `HomeRow`). */
export interface HomeLayout {
  order: string[];
  hidden: string[];
}

export interface AlbumPage {
  album: AlbumSummary;
  coverLargeUrl: string | null;
  duration: number | null;
  copyright: string | null;
  tracks: Track[];
}

export interface ArtistPage {
  artist: ArtistSummary;
  topTracks: Track[];
  albums: AlbumSummary[];
  singles: AlbumSummary[];
}

export interface PlaylistPage {
  playlist: PlaylistSummary;
  /** Dans l'ordre de la playlist. */
  tracks: TrackEntry[];
}

export interface SearchResults {
  tracks: Track[];
  albums: AlbumSummary[];
  artists: ArtistSummary[];
  playlists: PlaylistSummary[];
}

export type Theme = "seaglass" | "studio" | "atelier" | "acidwave" | "surreal";

/** Raccourci de la barre latérale ; le titre d'une playlist est gardé pour l'afficher sans requête. */
export type Pin = { kind: "playlist"; uuid: string; title: string } | { kind: "likedTracks" };

export interface UiSettings {
  theme: Theme;
  home: HomeLayout;
  pins: Pin[];
  /** Couleur d'accent `#rrggbb` ; `null` : celle du thème. */
  accent: string | null;
}

/** Taille du cache mpv en Mio : seules ces valeurs sont acceptées par le backend. */
export type CacheSize = 32 | 64 | 128 | 256;

/** Réglages de lecture, stockés à part des réglages d'UI. */
export interface PlaybackSettings {
  cacheSize: CacheSize;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

export interface SessionInfo {
  userId: number;
  countryCode: string;
}

export interface Toast {
  level: "error" | "info";
  message: string;
}

export const commands = {
  authStatus: () => invoke<SessionInfo | null>("auth_status"),
  login: () => invoke<SessionInfo>("login"),
  logout: () => invoke<void>("logout"),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  album: (id: number) => invoke<AlbumPage>("album", { id }),
  artist: (id: number) => invoke<ArtistPage>("artist", { id }),
  playlist: (uuid: string) => invoke<PlaylistPage>("playlist", { uuid }),
  libraryTracks: () => invoke<TrackEntry[]>("library_tracks"),
  libraryAlbums: () => invoke<AlbumSummary[]>("library_albums"),
  libraryArtists: () => invoke<ArtistSummary[]>("library_artists"),
  libraryPlaylists: () => invoke<PlaylistSummary[]>("library_playlists"),
  home: () => invoke<HomeSection[]>("home"),
  /** Avec `shuffle`, active la lecture aléatoire et part d'un titre au hasard. */
  playTracks: (trackIds: TrackId[], startIndex: number, shuffle = false) =>
    invoke<void>("play_tracks", { trackIds, startIndex, shuffle }),
  playNext: (trackId: TrackId) => invoke<void>("play_next", { trackId }),
  addToQueue: (trackId: TrackId) => invoke<void>("add_to_queue", { trackId }),
  /** Plusieurs titres dans l'ordre : en tête de la file avec `next` (« Lire ensuite »), sinon à la fin. */
  enqueueTracks: (trackIds: TrackId[], next: boolean) =>
    invoke<void>("enqueue_tracks", { trackIds, next }),
  removeFromQueue: (index: number) => invoke<void>("remove_from_queue", { index }),
  next: () => invoke<void>("next"),
  previous: () => invoke<void>("previous"),
  togglePause: () => invoke<void>("toggle_pause"),
  seek: (seconds: number) => invoke<void>("seek", { seconds }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: Repeat) => invoke<void>("set_repeat", { mode }),
  /** 0 à 100 ; rétablit le son s'il était coupé (sauf à 0). */
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setMuted: (muted: boolean) => invoke<void>("set_muted", { muted }),
  playerState: () => invoke<PlayerState>("player_state"),
  queueState: () => invoke<QueueView>("queue_state"),
  uiSettings: () => invoke<UiSettings>("ui_settings"),
  setUiSettings: (settings: UiSettings) => invoke<void>("set_ui_settings", { settings }),
  /** Sections affichées, dans l'ordre de l'écran ; renvoie la disposition enregistrée. */
  setHomeLayout: (sections: { key: string; hidden: boolean }[]) =>
    invoke<HomeLayout>("set_home_layout", { sections }),
  resetHomeLayout: () => invoke<HomeLayout>("reset_home_layout"),
  /** Remplace les épingles ; renvoie la liste enregistrée (validée, sans doublons). */
  setPins: (pins: Pin[]) => invoke<Pin[]>("set_pins", { pins }),
  playbackSettings: () => invoke<PlaybackSettings>("playback_settings"),
  /** Enregistre et applique à chaud (sans recharger le titre en cours). */
  setPlaybackSettings: (settings: PlaybackSettings) =>
    invoke<void>("set_playback_settings", { settings }),
  /** État réel du lancement au démarrage, lu dans le système. */
  autostartEnabled: () => invoke<boolean>("autostart_enabled"),
  /** Renvoie l'état relu après changement. */
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  updateStatus: () => invoke<UpdateInfo | null>("update_status"),
  /** L'installeur ferme l'app puis la relance. */
  installUpdate: () => invoke<void>("install_update"),
};

export const events = {
  onPlayerState: (cb: (s: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("player-state", (e) => cb(e.payload)),
  onQueueState: (cb: (q: QueueView) => void): Promise<UnlistenFn> =>
    listen<QueueView>("queue-state", (e) => cb(e.payload)),
  onUpdateReady: (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>("update-ready", (e) => cb(e.payload)),
  onToast: (cb: (t: Toast) => void): Promise<UnlistenFn> =>
    listen<Toast>("toast", (e) => cb(e.payload)),
};
