// Catalogue des thèmes pour la page Apparence (#390) : mode clair / sombre, familles
// (variantes claire et sombre d'un même thème), ordre et filtres de la galerie. Logique
// pure, sans état : la présentation reste côté UI, le backend ne connaît que les identifiants.

import type { Theme } from "./ipc";

export type Scheme = "light" | "dark";

export interface ThemeInfo {
  id: Theme;
  scheme: Scheme;
  /** Identifiant de la famille : le thème lui-même, ou sa variante claire. */
  family: Theme;
}

/** Thèmes dans l'ordre de la galerie ; noms et descriptions dans `i18n/settings.ts`. */
export const THEME_INFO: ThemeInfo[] = [
  { id: "seaglass", scheme: "dark", family: "seaglass" },
  { id: "studio", scheme: "dark", family: "studio" },
  { id: "atelier", scheme: "light", family: "atelier" },
  { id: "acidwave", scheme: "dark", family: "acidwave" },
  { id: "surreal", scheme: "light", family: "surreal" },
  { id: "solarpunk", scheme: "light", family: "solarpunk" },
  { id: "cyberpunk", scheme: "dark", family: "cyberpunk" },
  { id: "neumorph", scheme: "light", family: "neumorph" },
  { id: "neumorph-dark", scheme: "dark", family: "neumorph" },
  { id: "terminal", scheme: "dark", family: "terminal" },
  { id: "riso", scheme: "light", family: "riso" },
  { id: "bauhaus", scheme: "light", family: "bauhaus" },
  { id: "aero", scheme: "light", family: "aero" },
  { id: "alloy", scheme: "light", family: "alloy" },
  { id: "alloy-dark", scheme: "dark", family: "alloy" },
  { id: "midnight", scheme: "dark", family: "midnight" },
  { id: "estampe", scheme: "light", family: "estampe" },
];

export const THEMES: Theme[] = THEME_INFO.map((t) => t.id);

const byId = new Map(THEME_INFO.map((t) => [t.id, t]));

export function themeInfo(id: Theme): ThemeInfo {
  return byId.get(id) ?? { id, scheme: "dark", family: id };
}

export function familyOf(id: Theme): Theme {
  return themeInfo(id).family;
}

/** Variantes d'une famille, la claire d'abord (ordre de `THEME_INFO`). */
export function variants(family: Theme): ThemeInfo[] {
  return THEME_INFO.filter((t) => t.family === family);
}

export type GalleryFilter = "all" | "favorites" | "light" | "dark";

export const GALLERY_FILTERS: GalleryFilter[] = ["all", "favorites", "light", "dark"];

/** Familles affichées par la galerie : filtrées, favoris en tête, sinon dans l'ordre du
 *  catalogue. Une famille qui a les deux variantes passe dans les deux filtres de mode. */
export function galleryFamilies(filter: GalleryFilter, favorites: Theme[]): Theme[] {
  const families = [...new Set(THEME_INFO.map((t) => t.family))];
  const isFavorite = (f: Theme) => favorites.includes(f);
  const shown = families.filter((f) => {
    if (filter === "favorites") return isFavorite(f);
    if (filter === "light" || filter === "dark") return variants(f).some((v) => v.scheme === filter);
    return true;
  });
  // Tri stable : l'ordre du catalogue est gardé à l'intérieur de chaque groupe.
  return [...shown.filter(isFavorite), ...shown.filter((f) => !isFavorite(f))];
}

/** Thème appliqué par un clic sur la vignette d'une famille : le thème courant s'il en fait
 *  partie, sinon la variante du filtre de mode, sinon celle du même mode que le thème courant,
 *  sinon la première. */
export function variantFor(family: Theme, current: Theme, filter: GalleryFilter): Theme {
  const list = variants(family);
  if (list.some((v) => v.id === current)) return current;
  const scheme = filter === "light" || filter === "dark" ? filter : themeInfo(current).scheme;
  return (list.find((v) => v.scheme === scheme) ?? list[0] ?? themeInfo(family)).id;
}

/** Ajoute ou retire une famille des favoris, sans toucher à l'ordre des autres. */
export function toggleFavorite(favorites: Theme[], family: Theme): Theme[] {
  return favorites.includes(family) ? favorites.filter((f) => f !== family) : [...favorites, family];
}
