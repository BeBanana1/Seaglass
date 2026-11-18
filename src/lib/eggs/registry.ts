// Clins d'œil (#399) : registre déclaratif et déclencheurs, logique pure sans DOM.
//
// Règles communes : des allusions, jamais un titre, un nom, une réplique ni une image repris
// tels quels ; aucun son, aucune requête réseau ; un effet de recherche dure une à deux
// secondes et ne se rejoue pas dans la session.

/** Clin d'œil d'un jour : un détail dans l'en-tête, toute la journée. */
export type DateEgg =
  | "foundation"
  | "mozart"
  | "groundhog"
  | "mountDoom"
  | "aprilFools"
  | "lv426"
  | "jazz"
  | "may4"
  | "towel"
  | "musicDay"
  | "swissDay"
  | "hobbits"
  | "timeCircuits"
  | "halloween"
  | "cecilia"
  | "winter"
  | "christmas"
  | "leapDay"
  | "friday13";

/** Dates fixes, `[clin d'œil, mois (1 à 12), jour]`, dans l'ordre de priorité : si deux
 *  clins d'œil tombent le même jour, le premier l'emporte. */
const FIXED: [DateEgg, number, number][] = [
  ["christmas", 12, 24],
  ["christmas", 12, 25],
  ["winter", 12, 21],
  ["halloween", 10, 31],
  ["foundation", 10, 29],
  ["timeCircuits", 10, 21],
  ["cecilia", 11, 22],
  ["hobbits", 9, 22],
  ["swissDay", 8, 1],
  ["musicDay", 6, 21],
  ["towel", 5, 25],
  ["may4", 5, 4],
  ["jazz", 4, 30],
  ["lv426", 4, 26],
  ["aprilFools", 4, 1],
  ["mountDoom", 3, 25],
  ["leapDay", 2, 29],
  ["groundhog", 2, 2],
  ["mozart", 1, 27],
];

/** Clin d'œil du jour (heure locale), `null` s'il n'y en a pas. Un seul par jour. */
export function eggForDate(date: Date): DateEgg | null {
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const fixed = FIXED.find(([, m, d]) => m === month && d === day);
  if (fixed) return fixed[0];
  if (day === 13 && date.getDay() === 5) return "friday13";
  return null;
}

/** Clin d'œil d'une recherche : un effet bref, puis la recherche normale. */
export type SearchEgg =
  | "seaglass"
  | "rosebud"
  | "answer"
  | "playItAgain"
  | "silence"
  | "mellon"
  | "spice"
  | "origami"
  | "witcher"
  | "kessel";

/** Saisies reconnues, déjà normalisées (`normalize`). */
const QUERIES: Record<string, SearchEgg> = {
  seaglass: "seaglass",
  "verre de mer": "seaglass",
  rosebud: "rosebud",
  "42": "answer",
  "play it again": "playItAgain",
  silence: "silence",
  mellon: "mellon",
  epice: "spice",
  spice: "spice",
  origami: "origami",
  sorceleur: "witcher",
  witcher: "witcher",
  kessel: "kessel",
};

/** Minuscules, sans accents, espaces réduits : « Épice » et « épice » valent « epice ». */
export function normalize(query: string): string {
  return query.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().trim().replace(/\s+/g, " ");
}

/** Clin d'œil d'une saisie exacte (pas d'un préfixe ni d'un mot dans une phrase). */
export function eggForQuery(query: string): SearchEgg | null {
  return QUERIES[normalize(query)] ?? null;
}

/** Durée d'un effet de recherche, en millisecondes ; une phrase reste le temps d'être lue. */
export function searchEggMs(egg: SearchEgg): number {
  return egg === "answer" ? 4000 : 1800;
}

const ROMAN: [number, string][] = [
  [1000, "M"],
  [900, "CM"],
  [500, "D"],
  [400, "CD"],
  [100, "C"],
  [90, "XC"],
  [50, "L"],
  [40, "XL"],
  [10, "X"],
  [9, "IX"],
  [5, "V"],
  [4, "IV"],
  [1, "I"],
];

/** Chiffres romains ; zéro s'écrit « N » (nulla), comme dans les tables médiévales. */
export function roman(n: number): string {
  if (!Number.isInteger(n) || n < 0 || n >= 4000) return String(n);
  if (n === 0) return "N";
  let out = "";
  for (const [value, digits] of ROMAN) {
    while (n >= value) {
      out += digits;
      n -= value;
    }
  }
  return out;
}

/** Version en chiffres romains (1er avril) : `0.5.50` devient `N.V.L`. */
export function romanVersion(version: string): string {
  return version
    .split(".")
    .map((part) => (/^\d+$/.test(part) ? roman(Number(part)) : part))
    .join(".");
}

/** Date d'aperçu `AAAA-MM-JJ` (paramètre `?date=`), en heure locale ; invalide : `null`. */
export function parsePreviewDate(value: string | null): Date | null {
  const m = value?.match(/^(\d{4})-(\d{2})-(\d{2})$/);
  if (!m) return null;
  const date = new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return date.getMonth() === Number(m[2]) - 1 ? date : null;
}

const DATE_EGGS = new Set<string>([...FIXED.map(([egg]) => egg), "friday13"]);
const SEARCH_EGGS = new Set<string>(Object.values(QUERIES));

export function isDateEgg(id: string): id is DateEgg {
  return DATE_EGGS.has(id);
}

export function isSearchEgg(id: string): id is SearchEgg {
  return SEARCH_EGGS.has(id);
}
