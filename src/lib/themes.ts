// Theme catalog for the Appearance page (#390): light / dark mode, families (light and dark
// variants of the same theme), gallery order and filters. Pure logic, without state: the
// presentation stays in the UI, the backend only knows the identifiers.

import type { Theme } from "./ipc";

export type Scheme = "light" | "dark";

export interface ThemeInfo {
  id: Theme;
  scheme: Scheme;
  /** Identifier of the family: the theme itself, or its light variant. */
  family: Theme;
}

/** Themes in gallery order; names and descriptions in `i18n/settings/themes.ts`. */
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
  { id: "artdeco", scheme: "dark", family: "artdeco" },
  { id: "cassette", scheme: "dark", family: "cassette" },
  { id: "seance", scheme: "dark", family: "seance" },
  { id: "onboard", scheme: "dark", family: "onboard" },
  { id: "noir", scheme: "dark", family: "noir" },
  { id: "hotel", scheme: "light", family: "hotel" },
  { id: "desert", scheme: "light", family: "desert" },
  { id: "generique", scheme: "dark", family: "generique" },
];

export const THEMES: Theme[] = THEME_INFO.map((t) => t.id);

const byId = new Map(THEME_INFO.map((t) => [t.id, t]));

export function themeInfo(id: Theme): ThemeInfo {
  return byId.get(id) ?? { id, scheme: "dark", family: id };
}

export function familyOf(id: Theme): Theme {
  return themeInfo(id).family;
}

/** Variants of a family, the light one first (order of `THEME_INFO`). */
export function variants(family: Theme): ThemeInfo[] {
  return THEME_INFO.filter((t) => t.family === family);
}

export type GalleryFilter = "all" | "favorites" | "light" | "dark";

export const GALLERY_FILTERS: GalleryFilter[] = ["all", "favorites", "light", "dark"];

/** Families shown by the gallery: filtered, favorites first, otherwise in catalog order.
 *  A family that has both variants appears in both mode filters. */
export function galleryFamilies(filter: GalleryFilter, favorites: Theme[]): Theme[] {
  const families = [...new Set(THEME_INFO.map((t) => t.family))];
  const isFavorite = (f: Theme) => favorites.includes(f);
  const shown = families.filter((f) => {
    if (filter === "favorites") return isFavorite(f);
    if (filter === "light" || filter === "dark")
      return variants(f).some((v) => v.scheme === filter);
    return true;
  });
  // Stable sort: the catalog order is kept within each group.
  return [...shown.filter(isFavorite), ...shown.filter((f) => !isFavorite(f))];
}

/** Theme applied by a click on a family's thumbnail: the current theme if it belongs to it,
 *  otherwise the variant of the mode filter, otherwise the one with the same mode as the
 *  current theme, otherwise the first one. */
export function variantFor(family: Theme, current: Theme, filter: GalleryFilter): Theme {
  const list = variants(family);
  if (list.some((v) => v.id === current)) return current;
  const scheme = filter === "light" || filter === "dark" ? filter : themeInfo(current).scheme;
  return (list.find((v) => v.scheme === scheme) ?? list[0] ?? themeInfo(family)).id;
}

/** Adds or removes a family from the favorites, without changing the order of the others. */
export function toggleFavorite(favorites: Theme[], family: Theme): Theme[] {
  return favorites.includes(family)
    ? favorites.filter((f) => f !== family)
    : [...favorites, family];
}
