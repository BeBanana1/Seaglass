// Réglages d'interface (thème, couleur d'accent, disposition de l'accueil, épingles),
// enregistrés côté backend.

import {
  commands,
  type CloseAction,
  type HomeSection,
  type Pin,
  type Theme,
  type TrackSort,
  type UiSettings,
} from "../ipc";

/** Thèmes dans l'ordre du sélecteur ; noms et descriptions dans `i18n/settings.ts`. */
export const THEMES: Theme[] = ["seaglass", "studio", "atelier", "acidwave", "surreal", "solarpunk", "cyberpunk", "neumorph", "neumorph-dark", "terminal", "riso", "bauhaus", "aero", "alloy", "alloy-dark"];

/** Panneaux latéraux de la vue « En lecture » : file d'attente, paroles, ou les deux côte à côte
 *  (fenêtre large seulement, #119 ; ailleurs « both » montre les paroles). */
export type NowPlayingPanel = "queue" | "lyrics" | "both" | null;

export const ui = $state<{
  settings: UiSettings;
  /** Panneau ouvert dans la vue « En lecture » (mémorisé pour la session). */
  nowPlayingPanel: NowPlayingPanel;
}>({
  settings: {
    theme: "seaglass",
    home: { order: [], hidden: [], featuredHidden: false },
    pins: [],
    accent: null,
    language: "system",
    closeAction: "quit",
    discord: false,
    sorts: [],
  },
  nowPlayingPanel: null,
});

/** Couleurs proposées pour l'accent, en plus du sélecteur libre. */
export const ACCENT_PRESETS = [
  "#c8ff00",
  "#3ddc97",
  "#4cc9f0",
  "#7c9cff",
  "#b388ff",
  "#ff5c8a",
  "#ff7a45",
  "#ffb547",
];

/** Texte posé sur l'accent (bouton Lecture…) : noir sur une couleur claire, sinon blanc. */
function contrastOn(hex: string): string {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  // Seuil où le contraste avec le noir et avec le blanc s'égalisent.
  return luminance > 0.179 ? "#0a0a0b" : "#ffffff";
}

function apply() {
  const root = document.documentElement;
  root.dataset.theme = ui.settings.theme;
  // Le style en ligne de <html> l'emporte sur les jetons du thème ; sans accent choisi,
  // on le retire pour revenir à ceux du thème.
  const accent = ui.settings.accent;
  if (accent) {
    root.style.setProperty("--accent", accent);
    // Variante de survol : mêlée à la couleur du texte (plus claire sur fond sombre,
    // plus foncée sur Atelier).
    root.style.setProperty("--accent-strong", `color-mix(in oklab, ${accent} 75%, var(--text))`);
    root.style.setProperty("--accent-contrast", contrastOn(accent));
  } else {
    for (const name of ["--accent", "--accent-strong", "--accent-contrast"]) {
      root.style.removeProperty(name);
    }
  }
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

/** `null` : revient à l'accent du thème. */
export async function setAccent(accent: string | null) {
  ui.settings = { ...ui.settings, accent };
  apply();
  await commands.setUiSettings(ui.settings);
}

/** Présence Discord (#215), appliquée aussitôt par le backend. */
export async function setDiscord(discord: boolean) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, discord };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** Réglage « Fermer la fenêtre » (#83), appliqué par le backend à la fermeture. */
export async function setCloseAction(closeAction: CloseAction) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, closeAction };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** Enregistre l'ordre et le masquage des sections de l'accueil, tels qu'affichés. */
export async function saveHomeLayout(sections: Pick<HomeSection, "key" | "hidden">[]) {
  // Copie de la disposition gardée à jour : `setUiSettings` renvoie tout l'objet.
  ui.settings.home = await commands.setHomeLayout(
    sections.map(({ key, hidden }) => ({ key, hidden })),
  );
}

export async function setHomeFeatured(shown: boolean) {
  ui.settings.home = await commands.setHomeFeatured(shown);
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

/** Tri mémorisé d'une liste (uuid de playlist ou `likedTracks`, #93), sinon `fallback`. */
export function listSort(list: string, fallback: TrackSort): TrackSort {
  const saved = ui.settings.sorts.find((s) => s.list === list);
  return saved ? { key: saved.key, descending: saved.descending } : fallback;
}

/** Mémorise le tri choisi ; le backend oublie le tri par défaut. */
export async function saveListSort(list: string, sort: TrackSort) {
  ui.settings.sorts = await commands.setListSort(list, sort);
}
