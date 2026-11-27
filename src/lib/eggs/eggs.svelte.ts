// Easter eggs (#399): current state and triggering. Pure presentation, no backend:
// the `easterEggs` setting (Settings › Appearance) turns them all off.
//
// `<html>` carries `data-eggs` (setting on), `data-date-egg` (all day long),
// `data-egg` (search effect, one to two seconds), `data-egg-pixels` (keyboard sequence)
// and `data-egg-walk` (Desert theme): the components concerned use them in their CSS.

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

/** Duration of the big-pixel covers after the keyboard sequence. */
const PIXELS_MS = 10_000;
/** Desert theme: bars without a regular rhythm at the start of the first track of an album
 *  played as an album (first track played from that album, not necessarily track 1). */
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

/** Easter eggs visible now, setting included. */
export const eggs = {
  get date(): DateEgg | null {
    return ui.settings.easterEggs ? state.today : null;
  },
  get search(): SearchEgg | null {
    return ui.settings.easterEggs ? state.flash : null;
  },
  /** Same track played 10 times in a row on Repeat One: ∞ on the repeat icon. */
  get infinite(): boolean {
    return ui.settings.easterEggs && state.infinite;
  },
};

/** Search effects already played in the session: never twice. */
const played = new Set<SearchEgg>();
let flashTimer: ReturnType<typeof setTimeout> | undefined;
let pixelsTimer: ReturnType<typeof setTimeout> | undefined;
let loops = NO_LOOPS;
/** Previous track and album, to spot the entry into a new album. */
let lastTrack: number | null = null;
let lastAlbum: number | null = null;
let albumStart = false;
let progress = 0;

/** Preview outside Tauri (`pnpm dev`): `?date=YYYY-MM-DD` fixes the day, `?egg=<id>` forces a
 *  date egg, plays a search effect at startup, or `infinity` / `pixels`. */
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

/** Starts tracking the day, playback and keyboard, and the mirror on `<html>`. */
export function initEggs() {
  state.today = today();
  // The day changes while the app stays open: checked every minute.
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
    // A single read of `player.state` (~4 Hz); the flags rarely change.
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

/** Search started: plays its egg if there is one and it has not already played. */
export function searched(query: string) {
  const egg = eggForQuery(query);
  if (egg) play(egg);
}
