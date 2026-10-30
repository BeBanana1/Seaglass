// Langue de l'interface (#7). La langue effective est décidée par le backend (réglage, ou
// langue d'affichage du système) ; l'UI choisit seulement le dictionnaire à afficher.
//
// Les textes sont rangés par domaine dans `i18n/` : chaque fichier exporte `fr` (la source)
// et `en`, typé sur `fr` : une clé oubliée ou en trop fait échouer `pnpm check`.

import { commands, type Lang, type LanguagePref } from "./ipc";
import * as common from "./i18n/common";
import * as format from "./i18n/format";
import * as library from "./i18n/library";
import * as menu from "./i18n/menu";
import * as player from "./i18n/player";
import * as settings from "./i18n/settings";
import * as info from "./i18n/info";
import * as views from "./i18n/views";
import { i18n } from "./i18n/state.svelte";

export { i18n, locale, num } from "./i18n/state.svelte";

const fr = {
  common: common.fr,
  format: format.fr,
  library: library.fr,
  menu: menu.fr,
  player: player.fr,
  settings: settings.fr,
  info: info.fr,
  views: views.fr,
};

export type Dict = typeof fr;

const en: Dict = {
  common: common.en,
  format: format.en,
  library: library.en,
  menu: menu.en,
  player: player.en,
  settings: settings.en,
  info: info.en,
  views: views.en,
};

const dicts: Record<Lang, Dict> = { fr, en };

/** Textes dans la langue courante : `t.player.play`, `t.common.tracks(12)`. Lu dans un
 *  gabarit ou un `$derived`, il suit les changements de langue. */
export const t: Dict = new Proxy({} as Dict, {
  get: (_, key) => dicts[i18n.lang][key as keyof Dict],
});

function apply(lang: Lang) {
  i18n.lang = lang;
  document.documentElement.lang = lang;
}

export async function initLanguage() {
  try {
    apply(await commands.language());
  } catch {
    // Français si le backend ne répond pas.
  }
}

/** Enregistre le réglage ; le backend renvoie la langue effective. */
export async function setLanguage(pref: LanguagePref) {
  apply(await commands.setLanguage(pref));
}
