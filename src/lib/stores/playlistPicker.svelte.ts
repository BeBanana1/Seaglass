// "Add to playlist…" dialog (#4), shown by components/PlaylistPicker.svelte.

import type { TrackId } from "../ipc";

export interface PickerRequest {
  trackIds: TrackId[];
  /** What is being added, for the header ("Cornfield Chase", "30 tracks"). */
  label: string;
}

export const picker = $state<{ request: PickerRequest | null }>({ request: null });

export function openPlaylistPicker(request: PickerRequest) {
  picker.request = request;
}

export function closePlaylistPicker() {
  picker.request = null;
}

/** Last playlist changed: its page, if open, reloads (lib/stores, Playlist view). */
export const playlistChanges = $state<{ uuid: string | null; revision: number }>({
  uuid: null,
  revision: 0,
});

export function notePlaylistChange(uuid: string) {
  playlistChanges.uuid = uuid;
  playlistChanges.revision += 1;
}
