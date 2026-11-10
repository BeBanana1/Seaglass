// Mode conduite (#193, Android) : écran, tuiles, bouton d'épingle.

export const fr = {
  open: "Mode conduite",
  close: "Quitter le mode conduite",
  nothing: "Rien en lecture",
  empty:
    "Aucune tuile. Épinglez une playlist, un mix, vos titres likés ou « Plus tard » avec le bouton Épingler de leur page.",
  pin: "Ajouter au mode conduite",
  unpin: "Retirer du mode conduite",
  pinned: (label: string) => `« ${label} » ajouté au mode conduite`,
  full: (max: number) => `Le mode conduite a déjà ${max} tuiles : retirez-en une d'abord`,
  moveUp: "Avancer",
  moveDown: "Reculer",
  remove: "Retirer",
};

export const en: typeof fr = {
  open: "Driving mode",
  close: "Exit driving mode",
  nothing: "Nothing playing",
  empty: "No tiles yet. Pin a playlist, a mix, your liked tracks or “Later” with the Pin button on their page.",
  pin: "Add to driving mode",
  unpin: "Remove from driving mode",
  pinned: (label: string) => `“${label}” added to driving mode`,
  full: (max: number) => `Driving mode already has ${max} tiles: remove one first`,
  moveUp: "Move earlier",
  moveDown: "Move later",
  remove: "Remove",
};
