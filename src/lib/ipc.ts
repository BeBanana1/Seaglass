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
  title: string;
  items: MediaItem[];
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
  tracks: Track[];
}

export interface SearchResults {
  tracks: Track[];
  albums: AlbumSummary[];
  artists: ArtistSummary[];
  playlists: PlaylistSummary[];
}

export type Theme = "seaglass" | "studio" | "atelier" | "acidwave" | "surreal";

export interface UiSettings {
  theme: Theme;
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
  libraryTracks: () => invoke<Track[]>("library_tracks"),
  libraryAlbums: () => invoke<AlbumSummary[]>("library_albums"),
  libraryArtists: () => invoke<ArtistSummary[]>("library_artists"),
  libraryPlaylists: () => invoke<PlaylistSummary[]>("library_playlists"),
  home: () => invoke<HomeRow[]>("home"),
  /** Avec `shuffle`, active la lecture aléatoire et part d'un titre au hasard. */
  playTracks: (trackIds: TrackId[], startIndex: number, shuffle = false) =>
    invoke<void>("play_tracks", { trackIds, startIndex, shuffle }),
  playNext: (trackId: TrackId) => invoke<void>("play_next", { trackId }),
  addToQueue: (trackId: TrackId) => invoke<void>("add_to_queue", { trackId }),
  removeFromQueue: (index: number) => invoke<void>("remove_from_queue", { index }),
  next: () => invoke<void>("next"),
  previous: () => invoke<void>("previous"),
  togglePause: () => invoke<void>("toggle_pause"),
  seek: (seconds: number) => invoke<void>("seek", { seconds }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: Repeat) => invoke<void>("set_repeat", { mode }),
  playerState: () => invoke<PlayerState>("player_state"),
  queueState: () => invoke<QueueView>("queue_state"),
  uiSettings: () => invoke<UiSettings>("ui_settings"),
  setUiSettings: (settings: UiSettings) => invoke<void>("set_ui_settings", { settings }),
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
