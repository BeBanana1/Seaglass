// Interface language (#7). The backend decides the effective language (the setting, or the
// system's display language); the UI only picks the dictionary to show.
//
// Strings are grouped by area in `i18n/`: each file exports `fr` (the source) and `en`, typed
// on `fr`: a missing or extra key fails `pnpm check`.

import { commands, type Lang, type LanguagePref } from "./ipc";
import * as blindTest from "./i18n/blindTest";
import * as common from "./i18n/common";
import * as drive from "./i18n/drive";
import * as eggs from "./i18n/eggs";
import * as equalizer from "./i18n/equalizer";
import * as format from "./i18n/format";
import * as library from "./i18n/library";
import * as menu from "./i18n/menu";
import * as palette from "./i18n/palette";
import * as player from "./i18n/player";
import * as settings from "./i18n/settings";
import * as info from "./i18n/info";
import * as jam from "./i18n/jam";
import * as remote from "./i18n/remote";
import * as handoff from "./i18n/handoff";
import * as views from "./i18n/views";
import { i18n } from "./i18n/state.svelte";

export { i18n, locale, num } from "./i18n/state.svelte";

const fr = {
  blindTest: blindTest.fr,
  common: common.fr,
  drive: drive.fr,
  eggs: eggs.fr,
  equalizer: equalizer.fr,
  format: format.fr,
  library: library.fr,
  menu: menu.fr,
  palette: palette.fr,
  player: player.fr,
  settings: settings.fr,
  info: info.fr,
  jam: jam.fr,
  remote: remote.fr,
  handoff: handoff.fr,
  views: views.fr,
};

export type Dict = typeof fr;

const en: Dict = {
  blindTest: blindTest.en,
  common: common.en,
  drive: drive.en,
  eggs: eggs.en,
  equalizer: equalizer.en,
  format: format.en,
  library: library.en,
  menu: menu.en,
  palette: palette.en,
  player: player.en,
  settings: settings.en,
  info: info.en,
  jam: jam.en,
  remote: remote.en,
  handoff: handoff.en,
  views: views.en,
};

const dicts: Record<Lang, Dict> = { fr, en };

/** Strings in the current language: `t.player.play`, `t.common.tracks(12)`. Read in a
 *  template or a `$derived`, it follows language changes. */
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
    // French if the backend does not answer.
  }
}

/** Saves the setting; the backend returns the effective language. */
export async function setLanguage(pref: LanguagePref) {
  apply(await commands.setLanguage(pref));
}
