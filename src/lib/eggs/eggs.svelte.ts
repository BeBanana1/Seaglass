// Clins d'œil (#399) : état courant et déclenchement. Présentation pure, sans backend :
// le réglage `easterEggs` (Réglages › Apparence) les coupe tous.
//
// `<html>` porte `data-date-egg` (toute la journée) et `data-egg` (effet de recherche, une
// à deux secondes) : les composants concernés s'en servent dans leur CSS.

import { ui } from "../stores/ui.svelte";
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

const state = $state<{ today: DateEgg | null; flash: SearchEgg | null }>({
  today: null,
  flash: null,
});

/** Clins d'œil visibles maintenant, réglage compris. */
export const eggs = {
  get date(): DateEgg | null {
    return ui.settings.easterEggs ? state.today : null;
  },
  get search(): SearchEgg | null {
    return ui.settings.easterEggs ? state.flash : null;
  },
};

/** Effets de recherche déjà joués dans la session : jamais deux fois. */
const played = new Set<SearchEgg>();
let flashTimer: ReturnType<typeof setTimeout> | undefined;

/** Aperçu hors Tauri (`pnpm dev`) : `?date=AAAA-MM-JJ` fixe le jour, `?egg=<id>` force un
 *  clin d'œil de date ou joue un effet de recherche au démarrage. */
const preview = import.meta.env.DEV ? new URLSearchParams(location.search) : null;
const previewDate = parsePreviewDate(preview?.get("date") ?? null);
const previewEgg = preview?.get("egg") ?? null;

function today(): DateEgg | null {
  if (previewEgg && isDateEgg(previewEgg)) return previewEgg;
  return eggForDate(previewDate ?? new Date());
}

/** Lance le suivi du jour et le reflet sur `<html>`. */
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
      const date = eggs.date;
      if (date) root.dataset.dateEgg = date;
      else delete root.dataset.dateEgg;
      const flash = eggs.search;
      if (flash) root.dataset.egg = flash;
      else delete root.dataset.egg;
    });
  });
  if (previewEgg && isSearchEgg(previewEgg)) setTimeout(() => play(previewEgg, true), 800);
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
