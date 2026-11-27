// Command palette (#412): open or not. Desktop only.

export const palette = $state<{ open: boolean }>({ open: false });

export function openPalette() {
  palette.open = true;
}

export function closePalette() {
  palette.open = false;
}

export function togglePalette() {
  palette.open = !palette.open;
}

/** Playback settings changed outside the Settings page (exclusive mode from the palette):
 *  the page, if open, reads them again so as not to write back a stale copy. */
export const playbackSettingsChanged = $state<{ count: number }>({ count: 0 });
