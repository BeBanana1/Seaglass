// Fenêtre « Ajouter à une playlist » (#4), affichée par components/PlaylistPicker.svelte.

import type { TrackId } from "../ipc";

export interface PickerRequest {
  trackIds: TrackId[];
  /** Ce qui est ajouté, pour l'en-tête (« Cornfield Chase », « 30 titres »). */
  label: string;
}

export const picker = $state<{ request: PickerRequest | null }>({ request: null });

export function openPlaylistPicker(request: PickerRequest) {
  picker.request = request;
}

export function closePlaylistPicker() {
  picker.request = null;
}

/** Dernière playlist modifiée : sa page ouverte se recharge (lib/stores, vue Playlist). */
export const playlistChanges = $state<{ uuid: string | null; revision: number }>({
  uuid: null,
  revision: 0,
});

export function notePlaylistChange(uuid: string) {
  playlistChanges.uuid = uuid;
  playlistChanges.revision += 1;
}
