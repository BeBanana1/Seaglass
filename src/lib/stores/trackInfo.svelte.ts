// Panneau « Infos du titre » (#3), affiché par components/TrackInfo.svelte.

import type { Track } from "../ipc";

export const trackInfo = $state<{ track: Track | null }>({ track: null });

export function openTrackInfo(track: Track) {
  trackInfo.track = track;
}

export function closeTrackInfo() {
  trackInfo.track = null;
}
