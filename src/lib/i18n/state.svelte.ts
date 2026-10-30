// Langue courante et mise en forme des nombres. À part de `i18n.svelte.ts` pour que les
// dictionnaires puissent l'utiliser sans import circulaire.

import type { Lang } from "../ipc";

export const i18n = $state<{ lang: Lang }>({ lang: "fr" });

/** Locale de `Intl` (nombres, dates, tri) pour la langue courante. */
export function locale(): string {
  return i18n.lang === "en" ? "en-US" : "fr-FR";
}

/** Nombre mis en forme : « 1 234,5 » ou « 1,234.5 ». */
export function num(n: number, options?: Intl.NumberFormatOptions): string {
  return n.toLocaleString(locale(), options);
}
