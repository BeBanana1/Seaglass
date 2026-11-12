// Palette de commandes (#412) : filtre et classement des entrées. Logique pure, testée
// (`palette.test.ts`) ; les actions elles-mêmes sont dans `actions.ts`.

export interface PaletteItem {
  label: string;
  /** Autres mots qui doivent la trouver (synonymes, nom anglais, raccourci). */
  keywords?: string[];
}

/** Minuscules sans accents : « Réglages » et « reglages » se valent. */
export function normalize(text: string): string {
  return text.normalize("NFD").replace(/\p{M}/gu, "").toLowerCase();
}

function words(text: string): string[] {
  return normalize(text)
    .split(/[^\p{L}\p{N}+]+/u)
    .filter((w) => w !== "");
}

/** Score d'une entrée pour la requête, `null` si elle ne correspond pas. Chaque mot de la
 *  requête doit se retrouver, dans n'importe quel ordre : au début d'un mot du libellé (le
 *  mieux), au début d'un mot-clé, ou au milieu du libellé. Un libellé qui commence par la
 *  requête entière passe devant. */
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

/** Entrées qui correspondent, les meilleures d'abord ; à égalité, l'ordre d'origine. Requête
 *  vide : toutes, dans l'ordre d'origine. */
export function rank<T extends PaletteItem>(items: T[], query: string): T[] {
  return items
    .map((item, index) => ({ item, index, score: score(item, query) }))
    .filter((e): e is { item: T; index: number; score: number } => e.score !== null)
    .sort((a, b) => b.score - a.score || a.index - b.index)
    .map((e) => e.item);
}
