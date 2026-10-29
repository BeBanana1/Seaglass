// Wrappers typés des commandes et événements Tauri.
// Types dupliqués depuis Rust : tidal/models.rs, player/state.rs, mpv/events.rs.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

export type TrackId = number;

export interface ArtistRef {
  id: number;
  name: string;
}

export interface AlbumRef {
  id: number;
  title: string;
  coverUrl: string | null;
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
  search: (query: string) => invoke<Track[]>("search", { query }),
  play: (trackId: TrackId) => invoke<void>("play", { trackId }),
  togglePause: () => invoke<void>("toggle_pause"),
  seek: (seconds: number) => invoke<void>("seek", { seconds }),
  playerState: () => invoke<PlayerState>("player_state"),
};

export const events = {
  onPlayerState: (cb: (s: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("player-state", (e) => cb(e.payload)),
  onToast: (cb: (t: Toast) => void): Promise<UnlistenFn> =>
    listen<Toast>("toast", (e) => cb(e.payload)),
};
