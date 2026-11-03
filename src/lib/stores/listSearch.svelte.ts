// Mode recherche d'une liste de titres sur Android (#151) : tant que le filtre d'une
// playlist ou des titres likés a le focus ou un texte, la page masque son en-tête et le
// tri pour laisser la place aux résultats (le clavier prend la moitié de l'écran).
// Écrit par `TrackTools`, lu par les vues et par Retour (`App`). Jamais actif sur le bureau.

export const listSearch = $state({ active: false });

let exit: (() => void) | null = null;

/** Appelé par `TrackTools` ; `onExit` vide le champ et lui retire le focus. */
export function setListSearch(active: boolean, onExit: (() => void) | null) {
  listSearch.active = active;
  exit = active ? onExit : null;
}

/** Quitte le mode recherche (Retour d'Android). */
export function exitListSearch() {
  exit?.();
}
