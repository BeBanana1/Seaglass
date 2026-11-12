// Palette de commandes (#412) : ouverte ou non. Bureau seulement.

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

/** Réglages de lecture changés hors de la page Réglages (mode exclusif depuis la palette) :
 *  la page, si elle est ouverte, les relit pour ne pas réécrire une copie périmée. */
export const playbackSettingsChanged = $state<{ count: number }>({ count: 0 });
