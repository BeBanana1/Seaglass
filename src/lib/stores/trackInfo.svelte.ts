// "Track info" panel (#3), shown by components/TrackInfo.svelte.

import type { Track } from "../ipc";

export const trackInfo = $state<{ track: Track | null }>({ track: null });

export function openTrackInfo(track: Track) {
  trackInfo.track = track;
}

export function closeTrackInfo() {
  trackInfo.track = null;
}
