// Clins d'œil (#399) : état courant et déclenchement. Présentation pure, sans backend :
// le réglage `easterEggs` (Réglages › Apparence) les coupe tous.
//
// `<html>` porte `data-eggs` (réglage actif), `data-date-egg` (toute la journée),
// `data-egg` (effet de recherche, une à deux secondes), `data-egg-pixels` (séquence au
// clavier) et `data-egg-walk` (thème Désert) : les composants concernés s'en servent dans
// leur CSS.

import { untrack } from "svelte";
import { player } from "../stores/player.svelte";
import { queue } from "../stores/queue.svelte";
import { ui } from "../stores/ui.svelte";
import { countLoops, LOOPS_FOR_INFINITY, NO_LOOPS, SEQUENCE, sequenceStep } from "./playback";
import {
  eggForDate,
  eggForQuery,
  isDateEgg,
  isSearchEgg,
  parsePreviewDate,
  searchEggMs,
  type DateEgg,
  type SearchEgg,
} from "./registry";

export type { DateEgg, SearchEgg } from "./registry";

/** Durée des pochettes en gros pixels après la séquence au clavier. */
const PIXELS_MS = 10_000;
/** Thème Désert : barres sans rythme régulier au début du premier titre d'un album lu comme
 *  album (premier titre joué de cet album, pas forcément la piste 1). */
const WALK_SECONDS = 10;

const state = $state<{
  today: DateEgg | null;
  flash: SearchEgg | null;
  infinite: boolean;
  pixels: boolean;
  walk: boolean;
}>({
  today: null,
  flash: null,
  infinite: false,
  pixels: false,
  walk: false,
});

/** Clins d'œil visibles maintenant, réglage compris. */
export const eggs = {
  get date(): DateEgg | null {
    return ui.settings.easterEggs ? state.today : null;
  },
  get search(): SearchEgg | null {
    return ui.settings.easterEggs ? state.flash : null;
  },
  /** Même titre joué 10 fois de suite en Repeat One : ∞ sur l'icône de répétition. */
  get infinite(): boolean {
    return ui.settings.easterEggs && state.infinite;
  },
};

/** Effets de recherche déjà joués dans la session : jamais deux fois. */
const played = new Set<SearchEgg>();
let flashTimer: ReturnType<typeof setTimeout> | undefined;
let pixelsTimer: ReturnType<typeof setTimeout> | undefined;
let loops = NO_LOOPS;
/** Titre et album précédents, pour repérer l'entrée dans un nouvel album. */
let lastTrack: number | null = null;
let lastAlbum: number | null = null;
let albumStart = false;
let progress = 0;

/** Aperçu hors Tauri (`pnpm dev`) : `?date=AAAA-MM-JJ` fixe le jour, `?egg=<id>` force un
 *  clin d'œil de date, joue un effet de recherche au démarrage, ou `infinity` / `pixels`. */
const preview = import.meta.env.DEV ? new URLSearchParams(location.search) : null;
const previewDate = parsePreviewDate(preview?.get("date") ?? null);
const previewEgg = preview?.get("egg") ?? null;

function today(): DateEgg | null {
  if (previewEgg && isDateEgg(previewEgg)) return previewEgg;
  return eggForDate(previewDate ?? new Date());
}

function flag(name: string, on: boolean) {
  const root = document.documentElement;
  if (on) root.dataset[name] = "";
  else delete root.dataset[name];
}

/** Lance le suivi du jour, de la lecture et du clavier, et le reflet sur `<html>`. */
export function initEggs() {
  state.today = today();
  // Le jour change pendant que l'app reste ouverte : vérifié chaque minute.
  setInterval(() => {
    const egg = today();
    if (egg !== state.today) state.today = egg;
  }, 60_000);
  $effect.root(() => {
    $effect(() => {
      const root = document.documentElement;
      const on = ui.settings.easterEggs;
      flag("eggs", on);
      const date = eggs.date;
      if (date) root.dataset.dateEgg = date;
      else delete root.dataset.dateEgg;
      const flash = eggs.search;
      if (flash) root.dataset.egg = flash;
      else delete root.dataset.egg;
      flag("eggPixels", on && state.pixels);
      flag("eggWalk", on && state.walk);
    });
    // Une seule lecture de `player.state` (~4 Hz) ; les drapeaux ne changent que rarement.
    $effect(() => {
      const s = player.state;
      const id = s.track?.id ?? null;
      loops = countLoops(loops, id, s.timePos, s.duration, s.repeat === "one");
      const infinite = loops.count >= LOOPS_FOR_INFINITY || previewEgg === "infinity";
      if (infinite !== untrack(() => state.infinite)) state.infinite = infinite;
      if (s.track && s.track.id !== lastTrack) {
        albumStart = s.track.album.id !== lastAlbum;
        lastTrack = s.track.id;
        lastAlbum = s.track.album.id;
      }
      const walk =
        ui.settings.theme === "desert" &&
        albumStart &&
        queue.view.source?.kind === "album" &&
        !s.idle &&
        s.timePos < WALK_SECONDS;
      if (walk !== untrack(() => state.walk)) state.walk = walk;
    });
  });
  addEventListener("keydown", onKey);
  if (previewEgg && isSearchEgg(previewEgg)) setTimeout(() => play(previewEgg, true), 800);
  if (previewEgg === "pixels") pixelate();
}

function onKey(e: KeyboardEvent) {
  if (e.ctrlKey || e.altKey || e.metaKey) return;
  progress = sequenceStep(progress, e.key);
  if (progress === SEQUENCE.length) {
    progress = 0;
    if (ui.settings.easterEggs) pixelate();
  }
}

/** Pochettes en gros pixels quelques secondes (filtre SVG de `EggLayer.svelte`). */
function pixelate() {
  clearTimeout(pixelsTimer);
  state.pixels = true;
  pixelsTimer = setTimeout(() => (state.pixels = false), PIXELS_MS);
}

function play(egg: SearchEgg, again = false) {
  if (!ui.settings.easterEggs || (played.has(egg) && !again)) return;
  played.add(egg);
  clearTimeout(flashTimer);
  state.flash = egg;
  flashTimer = setTimeout(() => (state.flash = null), searchEggMs(egg));
}

/** Recherche lancée : joue son clin d'œil s'il y en a un et qu'il n'a pas déjà joué. */
export function searched(query: string) {
  const egg = eggForQuery(query);
  if (egg) play(egg);
}
