import { describe, expect, it } from "vitest";
import { normalize, rank, score } from "./palette";

const items = [
  { label: "Aller à l'accueil" },
  { label: "Ouvrir les réglages", keywords: ["settings", "préférences"] },
  { label: "Réglages › Lecture" },
  { label: "Thème : Studio" },
  { label: "Lecture aléatoire" },
];

describe("normalize", () => {
  it("ignore la casse et les accents", () => {
    expect(normalize("Réglages ÉTÉ")).toBe("reglages ete");
  });
});

describe("score", () => {
  it("une requête vide correspond à tout", () => {
    expect(score(items[0], "  ")).toBe(0);
  });

  it("refuse une entrée où un mot manque", () => {
    expect(score(items[3], "studio sombre")).toBeNull();
  });

  it("accepte les mots dans le désordre", () => {
    expect(score(items[2], "lecture reglages")).not.toBeNull();
  });

  it("trouve par mot-clé", () => {
    expect(score(items[1], "settings")).not.toBeNull();
    expect(score(items[1], "prefer")).not.toBeNull();
  });

  it("trouve au milieu d'un mot du libellé, moins bien qu'au début", () => {
    const middle = score(items[3], "udio");
    const start = score(items[3], "stud");
    expect(middle).not.toBeNull();
    expect(start!).toBeGreaterThan(middle!);
  });
});

describe("rank", () => {
  it("garde l'ordre d'origine pour une requête vide", () => {
    expect(rank(items, "")).toEqual(items);
  });

  it("met devant un libellé qui commence par la requête", () => {
    const ranked = rank(items, "lecture").map((i) => i.label);
    expect(ranked).toEqual(["Lecture aléatoire", "Réglages › Lecture"]);
  });

  it("garde l'ordre d'origine à égalité", () => {
    const ranked = rank(items, "reglages").map((i) => i.label);
    expect(ranked).toEqual(["Réglages › Lecture", "Ouvrir les réglages"]);
  });
});
