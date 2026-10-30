// Réglages d'interface (thème, disposition de l'accueil, épingles), enregistrés côté backend.

import { commands, type HomeSection, type Pin, type Theme, type UiSettings } from "../ipc";

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

export const ui = $state<{
  settings: UiSettings;
  /** Panneau de file dans la vue « En lecture » (mémorisé pour la session). */
  nowPlayingQueue: boolean;
}>({
  settings: { theme: "seaglass", home: { order: [], hidden: [] }, pins: [] },
  nowPlayingQueue: false,
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

/** Enregistre l'ordre et le masquage des sections de l'accueil, tels qu'affichés. */
export async function saveHomeLayout(sections: Pick<HomeSection, "key" | "hidden">[]) {
  // Copie de la disposition gardée à jour : `setUiSettings` renvoie tout l'objet.
  ui.settings.home = await commands.setHomeLayout(
    sections.map(({ key, hidden }) => ({ key, hidden })),
  );
}

export async function resetHomeLayout() {
  ui.settings.home = await commands.resetHomeLayout();
}

const samePin = (a: Pin, b: Pin) =>
  a.kind === b.kind && (a.kind !== "playlist" || (b.kind === "playlist" && a.uuid === b.uuid));

export function isPinned(pin: Pin): boolean {
  return ui.settings.pins.some((p) => samePin(p, pin));
}

async function savePins(pins: Pin[]) {
  ui.settings.pins = await commands.setPins(pins);
}

/** Épingle à la fin de la barre latérale, ou désépingle. */
export function togglePin(pin: Pin): Promise<void> {
  const pins = ui.settings.pins;
  return savePins(isPinned(pin) ? pins.filter((p) => !samePin(p, pin)) : [...pins, pin]);
}

/** Reporte le titre actuel d'une playlist épinglée (renommée depuis l'épinglage). */
export async function refreshPinTitle(uuid: string, title: string) {
  const pins = ui.settings.pins;
  const stale = pins.some((p) => p.kind === "playlist" && p.uuid === uuid && p.title !== title);
  if (!stale) return;
  await savePins(pins.map((p) => (p.kind === "playlist" && p.uuid === uuid ? { ...p, title } : p)));
}
