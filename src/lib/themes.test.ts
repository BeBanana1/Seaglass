import { describe, expect, it } from "vitest";
import {
  familyOf,
  galleryFamilies,
  THEME_INFO,
  toggleFavorite,
  variantFor,
  variants,
} from "./themes";

describe("theme catalog", () => {
  it("each family starts with a theme that belongs to it", () => {
    for (const t of THEME_INFO) expect(familyOf(t.family)).toBe(t.family);
  });

  it("groups the light and dark variants", () => {
    expect(variants("alloy").map((v) => v.id)).toEqual(["alloy", "alloy-dark"]);
    expect(familyOf("neumorph-dark")).toBe("neumorph");
    expect(variants("studio").map((v) => v.id)).toEqual(["studio"]);
  });
});

describe("galleryFamilies", () => {
  it("one thumbnail per family, in catalog order", () => {
    const all = galleryFamilies("all", []);
    expect(all).toHaveLength(23);
    expect(all).not.toContain("alloy-dark");
    expect(all.slice(0, 3)).toEqual(["seaglass", "studio", "atelier"]);
  });

  it("puts the favorites first, in catalog order", () => {
    const all = galleryFamilies("all", ["terminal", "studio"]);
    expect(all.slice(0, 3)).toEqual(["studio", "terminal", "seaglass"]);
  });

  it("filters the favorites", () => {
    expect(galleryFamilies("favorites", ["alloy", "atelier"])).toEqual(["atelier", "alloy"]);
    expect(galleryFamilies("favorites", [])).toEqual([]);
  });

  it("a family with both variants passes both mode filters", () => {
    const light = galleryFamilies("light", []);
    const dark = galleryFamilies("dark", []);
    expect(light).toContain("alloy");
    expect(dark).toContain("alloy");
    expect(light).not.toContain("studio");
    expect(dark).not.toContain("atelier");
  });
});

describe("variantFor", () => {
  it("keeps the current theme if it is in the family", () => {
    expect(variantFor("alloy", "alloy-dark", "light")).toBe("alloy-dark");
  });

  it("suit le filtre de mode", () => {
    expect(variantFor("neumorph", "atelier", "dark")).toBe("neumorph-dark");
  });

  it("otherwise the mode of the current theme", () => {
    expect(variantFor("alloy", "studio", "all")).toBe("alloy-dark");
    expect(variantFor("alloy", "atelier", "favorites")).toBe("alloy");
  });

  it("a theme with no variant stays itself", () => {
    expect(variantFor("studio", "atelier", "light")).toBe("studio");
  });
});

describe("toggleFavorite", () => {
  it("adds at the end, removes without moving the others", () => {
    expect(toggleFavorite(["studio"], "alloy")).toEqual(["studio", "alloy"]);
    expect(toggleFavorite(["studio", "alloy", "riso"], "alloy")).toEqual(["studio", "riso"]);
  });
});
