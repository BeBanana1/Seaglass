// Easter eggs (#399): declarative registry and triggers, pure logic without DOM.
//
// Common rules: allusions, never a title, a name, a line or an image taken over
// as is; no sound, no network request; a search effect lasts one to two
// seconds and does not replay in the session.

/** Easter egg of a day: a detail in the header, all day long. */
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

/** Fixed dates, `[egg, month (1 to 12), day]`, in priority order: if two
 *  eggs fall on the same day, the first wins. */
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

/** Egg of the day (local time), `null` if there is none. Only one per day. */
export function eggForDate(date: Date): DateEgg | null {
  const month = date.getMonth() + 1;
  const day = date.getDate();
  const fixed = FIXED.find(([, m, d]) => m === month && d === day);
  if (fixed) return fixed[0];
  if (day === 13 && date.getDay() === 5) return "friday13";
  return null;
}

/** Egg of a search: a brief effect, then the normal search. */
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

/** Recognized inputs, already normalized (`normalize`). */
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

/** Lowercase, no accents, collapsed spaces: "Épice" and "épice" are worth "epice". */
export function normalize(query: string): string {
  return query.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase().trim().replace(/\s+/g, " ");
}

/** Egg of an exact input (not of a prefix or a word in a sentence). */
export function eggForQuery(query: string): SearchEgg | null {
  return QUERIES[normalize(query)] ?? null;
}

/** Duration of a search effect, in milliseconds; a phrase stays long enough to be read. */
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

/** Roman numerals; zero is written "N" (nulla), as in medieval tables. */
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

/** Version in Roman numerals (April 1st): `0.5.50` becomes `N.V.L`. */
export function romanVersion(version: string): string {
  return version
    .split(".")
    .map((part) => (/^\d+$/.test(part) ? roman(Number(part)) : part))
    .join(".");
}

/** Preview date `YYYY-MM-DD` (`?date=` parameter), in local time; invalid: `null`. */
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
