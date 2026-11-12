// Réglages d'interface (thème, couleur d'accent, disposition de l'accueil, épingles),
// enregistrés côté backend.

import {
  commands,
  type CloseAction,
  type ContentFilter,
  type HomeSection,
  type Motion,
  type Pin,
  type Theme,
  type TrackSort,
  type UiSettings,
} from "../ipc";
import { motionReduced } from "../motion";
import { toggleFavorite } from "../themes";

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
    accents: {},
    favoriteThemes: [],
    language: "system",
    closeAction: "quit",
    discord: false,
    sorts: [],
    zoom: 100,
    motion: "system",
    content: { hideExplicit: false, hideAi: true },
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

/** Choix proposés pour les animations (#351). */
export const MOTIONS: Motion[] = ["system", "reduced", "full"];

/** « Réduire les animations » du système, suivi en direct. */
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

function applyMotion() {
  const reduced = motionReduced(ui.settings.motion, reducedMotion.matches);
  document.documentElement.dataset.motion = reduced ? "reduced" : "full";
}

function apply() {
  const root = document.documentElement;
  root.dataset.theme = ui.settings.theme;
  applyMotion();
  // Le style en ligne de <html> l'emporte sur les jetons du thème ; sans accent choisi,
  // on le retire pour revenir à ceux du thème.
  const accent = currentAccent();
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
  reducedMotion.addEventListener("change", applyMotion);
}

/** Animations de l'interface (#351), appliquées tout de suite. */
export async function setMotion(motion: Motion) {
  if (motion === ui.settings.motion) return;
  const previous = ui.settings;
  ui.settings = { ...ui.settings, motion };
  applyMotion();
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    applyMotion();
    throw e;
  }
}

export async function setTheme(theme: Theme) {
  ui.settings = { ...ui.settings, theme };
  apply();
  await commands.setUiSettings(ui.settings);
}

/** Couleur choisie pour le thème courant (#250), `null` : celle du thème. */
export function currentAccent(): string | null {
  return ui.settings.accents[ui.settings.theme] ?? null;
}

/** Pour le thème courant seulement ; `null` : revient à l'accent du thème. */
export async function setAccent(accent: string | null) {
  const accents = { ...ui.settings.accents };
  if (accent) accents[ui.settings.theme] = accent;
  else delete accents[ui.settings.theme];
  ui.settings = { ...ui.settings, accents };
  apply();
  await commands.setUiSettings(ui.settings);
}

/** Ajoute ou retire une famille de thèmes des favoris (#390). */
export async function toggleFavoriteTheme(family: Theme) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, favoriteThemes: toggleFavorite(ui.settings.favoriteThemes, family) };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
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

/** Filtre de contenu (#368), appliqué par le backend aux listes chargées ensuite. */
export async function setContentFilter(content: ContentFilter) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, content };
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

/** Tailles de l'interface (#337), les mêmes que `zoom::LEVELS` côté Rust. */
export const ZOOM_LEVELS = [80, 90, 100, 110, 125, 150];

/** Taille de l'interface, appliquée au zoom de la WebView par le backend. */
export async function setZoom(zoom: number) {
  if (zoom === ui.settings.zoom || !ZOOM_LEVELS.includes(zoom)) return;
  const previous = ui.settings;
  ui.settings = { ...ui.settings, zoom };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** Raccourcis Ctrl + « + » / « - » : niveau voisin ; 0 revient à 100 %. */
export function stepZoom(step: -1 | 0 | 1): Promise<void> {
  if (step === 0) return setZoom(100);
  const i = ZOOM_LEVELS.indexOf(ui.settings.zoom);
  const next = ZOOM_LEVELS[Math.min(ZOOM_LEVELS.length - 1, Math.max(0, (i < 0 ? 2 : i) + step))];
  return setZoom(next);
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

/** Identité d'une épingle (clé de liste) : deux épingles de même clé sont la même. */
export function pinKey(pin: Pin): string {
  if (pin.kind === "playlist") return `playlist:${pin.uuid}`;
  if (pin.kind === "mix") return `mix:${pin.id}`;
  if (pin.kind === "smart") return `smart:${pin.list}`;
  return pin.kind;
}

const samePin = (a: Pin, b: Pin) => pinKey(a) === pinKey(b);

/** Tuiles du mode conduite (#193) : les premières épingles, 8 au plus sur Android. */
export const MAX_DRIVE_TILES = 8;

export function isPinned(pin: Pin): boolean {
  return ui.settings.pins.some((p) => samePin(p, pin));
}

async function savePins(pins: Pin[]) {
  ui.settings.pins = await commands.setPins(pins);
}

/** Épingle à la fin de la barre latérale (ou des tuiles du mode conduite), ou désépingle.
 *  `limit` : nombre d'épingles au-delà duquel l'ajout est refusé (`false` renvoyé). */
export async function togglePin(pin: Pin, limit = Infinity): Promise<boolean> {
  const pins = ui.settings.pins;
  if (isPinned(pin)) {
    await savePins(pins.filter((p) => !samePin(p, pin)));
    return true;
  }
  if (pins.length >= limit) return false;
  await savePins([...pins, pin]);
  return true;
}

/** Avance (`-1`) ou recule (`1`) une épingle d'une place. */
export async function movePin(pin: Pin, delta: -1 | 1) {
  const pins = [...ui.settings.pins];
  const from = pins.findIndex((p) => samePin(p, pin));
  const to = from + delta;
  if (from < 0 || to < 0 || to >= pins.length) return;
  [pins[from], pins[to]] = [pins[to], pins[from]];
  await savePins(pins);
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
