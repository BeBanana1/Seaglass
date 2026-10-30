// Réglages d'interface (thème, couleur d'accent, disposition de l'accueil, épingles),
// enregistrés côté backend.

import { commands, type HomeSection, type Pin, type Theme, type UiSettings } from "../ipc";

/** Thèmes dans l'ordre du sélecteur ; noms et descriptions dans `i18n/settings.ts`. */
export const THEMES: Theme[] = ["seaglass", "studio", "atelier", "acidwave", "surreal", "solarpunk"];

/** Panneau latéral de la vue « En lecture » : file d'attente ou paroles, un seul à la fois. */
export type NowPlayingPanel = "queue" | "lyrics" | null;

export const ui = $state<{
  settings: UiSettings;
  /** Panneau ouvert dans la vue « En lecture » (mémorisé pour la session). */
  nowPlayingPanel: NowPlayingPanel;
}>({
  settings: { theme: "seaglass", home: { order: [], hidden: [] }, pins: [], accent: null, language: "system" },
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
