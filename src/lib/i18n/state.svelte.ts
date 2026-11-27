// Current language and number formatting. Apart from `i18n.svelte.ts` so that the
// dictionaries can use it without a circular import.

import type { Lang } from "../ipc";

export const i18n = $state<{ lang: Lang }>({ lang: "fr" });

/** `Intl` locale (numbers, dates, sorting) for the current language. */
export function locale(): string {
  return i18n.lang === "en" ? "en-US" : "fr-FR";
}

/** Formatted number: "1 234,5" or "1,234.5". */
export function num(n: number, options?: Intl.NumberFormatOptions): string {
  return n.toLocaleString(locale(), options);
}
