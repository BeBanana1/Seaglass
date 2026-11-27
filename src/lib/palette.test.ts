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
  it("ignores case and accents", () => {
    expect(normalize("Réglages ÉTÉ")).toBe("reglages ete");
  });
});

describe("score", () => {
  it("an empty query matches everything", () => {
    expect(score(items[0], "  ")).toBe(0);
  });

  it("refuses an entry where a word is missing", () => {
    expect(score(items[3], "studio sombre")).toBeNull();
  });

  it("accepts the words in any order", () => {
    expect(score(items[2], "lecture reglages")).not.toBeNull();
  });

  it("finds by keyword", () => {
    expect(score(items[1], "settings")).not.toBeNull();
    expect(score(items[1], "prefer")).not.toBeNull();
  });

  it("finds in the middle of a label word, less well than at the start", () => {
    const middle = score(items[3], "udio");
    const start = score(items[3], "stud");
    expect(middle).not.toBeNull();
    expect(start!).toBeGreaterThan(middle!);
  });
});

describe("rank", () => {
  it("keeps the original order for an empty query", () => {
    expect(rank(items, "")).toEqual(items);
  });

  it("puts a label that starts with the query first", () => {
    const ranked = rank(items, "lecture").map((i) => i.label);
    expect(ranked).toEqual(["Lecture aléatoire", "Réglages › Lecture"]);
  });

  it("keeps the original order on a tie", () => {
    const ranked = rank(items, "reglages").map((i) => i.label);
    expect(ranked).toEqual(["Réglages › Lecture", "Ouvrir les réglages"]);
  });
});
