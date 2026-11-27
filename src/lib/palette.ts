// Command palette (#412): filtering and ranking of the entries. Pure logic, tested
// (`palette.test.ts`); the actions themselves are in `actions.ts`.

export interface PaletteItem {
  label: string;
  /** Other words that should find it (synonyms, English name, shortcut). */
  keywords?: string[];
}

/** Lowercase without accents: "Réglages" and "reglages" are worth the same. */
export function normalize(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

function words(text: string): string[] {
  return normalize(text)
    .split(/[^\p{L}\p{N}+]+/u)
    .filter((w) => w !== "");
}

/** Score of an entry for the query, `null` if it does not match. Every word of the
 *  query must be found, in any order: at the start of a word of the label (the
 *  best), at the start of a keyword, or in the middle of the label. A label that starts with the
 *  whole query comes first. */
export function score(item: PaletteItem, query: string): number | null {
  const terms = words(query);
  if (terms.length === 0) return 0;
  const label = normalize(item.label);
  const labelWords = words(item.label);
  const keywordWords = (item.keywords ?? []).flatMap(words);
  let total = 0;
  for (const term of terms) {
    if (labelWords.some((w) => w.startsWith(term))) total += 3;
    else if (keywordWords.some((w) => w.startsWith(term))) total += 2;
    else if (label.includes(term)) total += 1;
    else return null;
  }
  if (label.startsWith(terms.join(" "))) total += 5;
  return total;
}

/** Entries that match, best first; on a tie, the original order. Empty query:
 *  all of them, in the original order. */
export function rank<T extends PaletteItem>(items: T[], query: string): T[] {
  return items
    .map((item, index) => ({ item, index, score: score(item, query) }))
    .filter((e): e is { item: T; index: number; score: number } => e.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((e) => e.item);
}
