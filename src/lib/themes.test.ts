import { describe, expect, it } from "vitest";
import { familyOf, galleryFamilies, THEME_INFO, toggleFavorite, variantFor, variants } from "./themes";

describe("catalogue des thèmes", () => {
  it("chaque famille commence par un thème qui en fait partie", () => {
    for (const t of THEME_INFO) expect(familyOf(t.family)).toBe(t.family);
  });

  it("regroupe les variantes claire et sombre", () => {
    expect(variants("alloy").map((v) => v.id)).toEqual(["alloy", "alloy-dark"]);
    expect(familyOf("neumorph-dark")).toBe("neumorph");
    expect(variants("studio").map((v) => v.id)).toEqual(["studio"]);
  });
});

describe("galleryFamilies", () => {
  it("une vignette par famille, dans l'ordre du catalogue", () => {
    const all = galleryFamilies("all", []);
    expect(all).toHaveLength(20);
    expect(all).not.toContain("alloy-dark");
    expect(all.slice(0, 3)).toEqual(["seaglass", "studio", "atelier"]);
  });

  it("met les favoris en tête, dans l'ordre du catalogue", () => {
    const all = galleryFamilies("all", ["terminal", "studio"]);
    expect(all.slice(0, 3)).toEqual(["studio", "terminal", "seaglass"]);
  });

  it("filtre les favoris", () => {
    expect(galleryFamilies("favorites", ["alloy", "atelier"])).toEqual(["atelier", "alloy"]);
    expect(galleryFamilies("favorites", [])).toEqual([]);
  });

  it("une famille aux deux variantes passe dans les deux filtres de mode", () => {
    const light = galleryFamilies("light", []);
    const dark = galleryFamilies("dark", []);
    expect(light).toContain("alloy");
    expect(dark).toContain("alloy");
    expect(light).not.toContain("studio");
    expect(dark).not.toContain("atelier");
  });
});

describe("variantFor", () => {
  it("garde le thème courant s'il est de la famille", () => {
    expect(variantFor("alloy", "alloy-dark", "light")).toBe("alloy-dark");
  });

  it("suit le filtre de mode", () => {
    expect(variantFor("neumorph", "atelier", "dark")).toBe("neumorph-dark");
  });

  it("sinon le mode du thème courant", () => {
    expect(variantFor("alloy", "studio", "all")).toBe("alloy-dark");
    expect(variantFor("alloy", "atelier", "favorites")).toBe("alloy");
  });

  it("un thème sans variante reste lui-même", () => {
    expect(variantFor("studio", "atelier", "light")).toBe("studio");
  });
});

describe("toggleFavorite", () => {
  it("ajoute à la fin, retire sans déplacer les autres", () => {
    expect(toggleFavorite(["studio"], "alloy")).toEqual(["studio", "alloy"]);
    expect(toggleFavorite(["studio", "alloy", "riso"], "alloy")).toEqual(["studio", "riso"]);
  });
});
