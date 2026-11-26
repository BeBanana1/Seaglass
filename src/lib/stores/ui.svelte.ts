// Interface settings (theme, accent color, home layout, pins), saved on the backend side.

import {
  commands,
  type CloseAction,
  type ContentFilter,
  type GlobalShortcuts,
  type HomeSection,
  type Motion,
  type Pin,
  type Theme,
  type TrackSort,
  type UiSettings,
} from "../ipc";
import { motionReduced } from "../motion";
import { toggleFavorite } from "../themes";

/** Side panels of the "Now playing" view: queue, lyrics, or both side by side (wide window
 *  only, #119; elsewhere "both" shows the lyrics). */
export type NowPlayingPanel = "queue" | "lyrics" | "both" | null;

export const ui = $state<{
  settings: UiSettings;
  /** Panel open in the "Now playing" view (remembered for the session). */
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
    easterEggs: true,
    content: { hideExplicit: false, hideAi: true },
    globalShortcuts: { enabled: false, bindings: {} },
    autoUpdates: true,
    jam: true,
    releaseNotifications: false,
  },
  nowPlayingPanel: null,
});

/** Colors offered for the accent, besides the free picker. */
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

/** Text on the accent (Play button…): black on a light color, otherwise white. */
function contrastOn(hex: string): string {
  const [r, g, b] = [1, 3, 5].map((i) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  });
  const luminance = 0.2126 * r + 0.7152 * g + 0.0722 * b;
  // Threshold where the contrast with black and with white are equal.
  return luminance > 0.179 ? "#0a0a0b" : "#ffffff";
}

/** Choices offered for animations (#351). */
export const MOTIONS: Motion[] = ["system", "reduced", "full"];

/** The system's "reduce motion" setting, followed live. */
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

function applyMotion() {
  const reduced = motionReduced(ui.settings.motion, reducedMotion.matches);
  document.documentElement.dataset.motion = reduced ? "reduced" : "full";
}

function apply() {
  const root = document.documentElement;
  root.dataset.theme = ui.settings.theme;
  applyMotion();
  // The inline style of <html> wins over the theme's tokens; with no accent chosen, it is
  // removed to fall back to the theme's.
  const accent = currentAccent();
  if (accent) {
    root.style.setProperty("--accent", accent);
    // Hover variant: mixed with the text color (lighter on a dark background, darker on
    // Atelier).
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
    // Default values if the backend does not answer.
  }
  apply();
  reducedMotion.addEventListener("change", applyMotion);
}

/** Interface animations (#351), applied right away. */
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

/** Color chosen for the current theme (#250), `null`: the theme's own. */
export function currentAccent(): string | null {
  return ui.settings.accents[ui.settings.theme] ?? null;
}

/** For the current theme only; `null`: back to the theme's accent. */
export async function setAccent(accent: string | null) {
  const accents = { ...ui.settings.accents };
  if (accent) accents[ui.settings.theme] = accent;
  else delete accents[ui.settings.theme];
  ui.settings = { ...ui.settings, accents };
  apply();
  await commands.setUiSettings(ui.settings);
}

/** Adds a theme family to the favorites or removes it (#390). */
export async function toggleFavoriteTheme(family: Theme) {
  const previous = ui.settings;
  ui.settings = {
    ...ui.settings,
    favoriteThemes: toggleFavorite(ui.settings.favoriteThemes, family),
  };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** Discord presence (#215), applied at once by the backend. */
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

/** Switches read by the backend: automatic updates, Jam (#452), new release notifications
 * (#435). */
export async function setUiToggle(
  key: "autoUpdates" | "jam" | "releaseNotifications",
  value: boolean,
) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, [key]: value };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** Easter eggs (#399): the date and the searches read them again at once. */
export async function setEasterEggs(easterEggs: boolean) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, easterEggs };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** Content filter (#368), applied by the backend to the lists loaded afterwards. */
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

/** Global shortcuts (#413): registered by the backend, which then reports those it could
 *  not take (`globalShortcutsStatus`). */
export async function setGlobalShortcuts(globalShortcuts: GlobalShortcuts) {
  const previous = ui.settings;
  ui.settings = { ...ui.settings, globalShortcuts };
  try {
    await commands.setUiSettings(ui.settings);
  } catch (e) {
    ui.settings = previous;
    throw e;
  }
}

/** "Close the window" setting (#83), applied by the backend on close. */
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

/** Interface sizes (#337), the same as `zoom::LEVELS` on the Rust side. */
export const ZOOM_LEVELS = [80, 90, 100, 110, 125, 150];

/** Interface size, applied to the WebView's zoom by the backend. */
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

/** Ctrl + "+" / "-" shortcuts: next level; 0 goes back to 100 %. */
export function stepZoom(step: -1 | 0 | 1): Promise<void> {
  if (step === 0) return setZoom(100);
  const i = ZOOM_LEVELS.indexOf(ui.settings.zoom);
  const next = ZOOM_LEVELS[Math.min(ZOOM_LEVELS.length - 1, Math.max(0, (i < 0 ? 2 : i) + step))];
  return setZoom(next);
}

/** Saves the order and hiding of the home sections, as shown. */
export async function saveHomeLayout(sections: Pick<HomeSection, "key" | "hidden">[]) {
  // Keeps the local copy of the layout current: `setUiSettings` sends the whole object.
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

/** Identity of a pin (list key): two pins with the same key are the same pin. */
export function pinKey(pin: Pin): string {
  if (pin.kind === "playlist") return `playlist:${pin.uuid}`;
  if (pin.kind === "mix") return `mix:${pin.id}`;
  if (pin.kind === "smart") return `smart:${pin.list}`;
  return pin.kind;
}

const samePin = (a: Pin, b: Pin) => pinKey(a) === pinKey(b);

/** Driving mode tiles (#193): the first pins, 8 at most on Android. */
export const MAX_DRIVE_TILES = 8;

export function isPinned(pin: Pin): boolean {
  return ui.settings.pins.some((p) => samePin(p, pin));
}

async function savePins(pins: Pin[]) {
  ui.settings.pins = await commands.setPins(pins);
}

/** Pins at the end of the sidebar (or of the driving mode tiles), or unpins.
 *  `limit`: number of pins past which adding is refused (`false` returned). */
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

/** Moves a pin one place up (`-1`) or down (`1`). */
export async function movePin(pin: Pin, delta: -1 | 1) {
  const pins = [...ui.settings.pins];
  const from = pins.findIndex((p) => samePin(p, pin));
  const to = from + delta;
  if (from < 0 || to < 0 || to >= pins.length) return;
  [pins[from], pins[to]] = [pins[to], pins[from]];
  await savePins(pins);
}

/** Copies the current title of a pinned playlist (renamed since it was pinned). */
export async function refreshPinTitle(uuid: string, title: string) {
  const pins = ui.settings.pins;
  const stale = pins.some((p) => p.kind === "playlist" && p.uuid === uuid && p.title !== title);
  if (!stale) return;
  await savePins(pins.map((p) => (p.kind === "playlist" && p.uuid === uuid ? { ...p, title } : p)));
}

/** Remembered sort of a list (playlist uuid or `likedTracks`, #93), otherwise `fallback`. */
export function listSort(list: string, fallback: TrackSort): TrackSort {
  const saved = ui.settings.sorts.find((s) => s.list === list);
  return saved ? { key: saved.key, descending: saved.descending } : fallback;
}

/** Remembers the chosen sort; the backend forgets the default sort. */
export async function saveListSort(list: string, sort: TrackSort) {
  ui.settings.sorts = await commands.setListSort(list, sort);
}
