// Réglages d'interface (thème), enregistrés côté backend.

import { commands, type Theme, type UiSettings } from "../ipc";

export const THEMES: { id: Theme; name: string; description: string }[] = [
  {
    id: "seaglass",
    name: "Verre de mer",
    description: "Verre dépoli sur la pochette en cours ; les couleurs suivent l'album.",
  },
  {
    id: "studio",
    name: "Studio",
    description: "Console sombre et dense, la chaîne audio au premier plan.",
  },
  {
    id: "atelier",
    name: "Atelier",
    description: "Clair et éditorial, grande typographie, beaucoup d'air.",
  },
  {
    id: "acidwave",
    name: "Acidwave",
    description: "Néons acides, chrome liquide et hologrammes ; ça pulse pendant la lecture.",
  },
  {
    id: "surreal",
    name: "Surréalisme",
    description: "Paysage vectoriel onirique teinté par l'album, objets qui flottent.",
  },
];

export const ui = $state<{ settings: UiSettings; appearanceOpen: boolean }>({
  settings: { theme: "seaglass" },
  appearanceOpen: false,
});

function apply() {
  document.documentElement.dataset.theme = ui.settings.theme;
}

export async function initUi() {
  try {
    ui.settings = await commands.uiSettings();
  } catch {
    // Valeurs par défaut si le backend ne répond pas.
  }
  apply();
}

export async function setTheme(theme: Theme) {
  ui.settings = { ...ui.settings, theme };
  apply();
  await commands.setUiSettings(ui.settings);
}
