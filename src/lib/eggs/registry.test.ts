import { describe, expect, it } from "vitest";
import {
  eggForDate,
  eggForQuery,
  isDateEgg,
  isSearchEgg,
  parsePreviewDate,
  roman,
  romanVersion,
} from "./registry";

const day = (y: number, m: number, d: number) => new Date(y, m - 1, d);

describe("eggForDate", () => {
  it("reconnaît les dates fixes", () => {
    expect(eggForDate(day(2027, 5, 4))).toBe("may4");
    expect(eggForDate(day(2026, 10, 31))).toBe("halloween");
    expect(eggForDate(day(2026, 8, 1))).toBe("swissDay");
    expect(eggForDate(day(2026, 12, 24))).toBe("christmas");
    expect(eggForDate(day(2026, 12, 25))).toBe("christmas");
    expect(eggForDate(day(2026, 12, 21))).toBe("winter");
    expect(eggForDate(day(2026, 10, 29))).toBe("foundation");
    expect(eggForDate(day(2026, 10, 21))).toBe("timeCircuits");
  });

  it("rien un jour ordinaire, ni la veille ou le lendemain", () => {
    expect(eggForDate(day(2026, 11, 12))).toBeNull();
    expect(eggForDate(day(2026, 5, 3))).toBeNull();
    expect(eggForDate(day(2026, 12, 26))).toBeNull();
  });

  it("passe le changement d'année", () => {
    expect(eggForDate(day(2026, 12, 31))).toBeNull();
    expect(eggForDate(day(2027, 1, 1))).toBeNull();
    expect(eggForDate(day(2027, 1, 27))).toBe("mozart");
  });

  it("vendredi 13 seulement un vendredi", () => {
    expect(eggForDate(day(2026, 11, 13))).toBe("friday13");
    expect(eggForDate(day(2026, 3, 13))).toBe("friday13");
    expect(eggForDate(day(2026, 12, 13))).toBeNull();
  });

  it("29 février les années bissextiles", () => {
    expect(eggForDate(day(2028, 2, 29))).toBe("leapDay");
    // 2027 n'est pas bissextile : le 29 février devient le 1er mars.
    expect(eggForDate(day(2027, 2, 29))).toBeNull();
  });
});

describe("eggForQuery", () => {
  it("ignore la casse, les accents et les espaces", () => {
    expect(eggForQuery("Seaglass")).toBe("seaglass");
    expect(eggForQuery("  verre   de MER ")).toBe("seaglass");
    expect(eggForQuery("Épice")).toBe("spice");
    expect(eggForQuery("epice")).toBe("spice");
    expect(eggForQuery("42")).toBe("answer");
    expect(eggForQuery("Kessel")).toBe("kessel");
  });

  it("une saisie exacte seulement", () => {
    expect(eggForQuery("4")).toBeNull();
    expect(eggForQuery("420")).toBeNull();
    expect(eggForQuery("rosebud remix")).toBeNull();
    expect(eggForQuery("")).toBeNull();
  });
});

describe("chiffres romains", () => {
  it("convertit", () => {
    expect(roman(0)).toBe("N");
    expect(roman(4)).toBe("IV");
    expect(roman(50)).toBe("L");
    expect(roman(1994)).toBe("MCMXCIV");
  });

  it("écrit une version", () => {
    expect(romanVersion("0.5.50")).toBe("N.V.L");
    expect(romanVersion("1.0.9")).toBe("I.N.IX");
  });
});

describe("parsePreviewDate", () => {
  it("lit AAAA-MM-JJ en heure locale", () => {
    expect(parsePreviewDate("2026-05-04")?.getDate()).toBe(4);
    expect(parsePreviewDate("2026-02-30")).toBeNull();
    expect(parsePreviewDate("4 mai")).toBeNull();
    expect(parsePreviewDate(null)).toBeNull();
  });
});

describe("identifiants", () => {
  it("sépare dates et recherches", () => {
    expect(isDateEgg("may4")).toBe(true);
    expect(isDateEgg("friday13")).toBe(true);
    expect(isSearchEgg("mellon")).toBe(true);
    expect(isSearchEgg("may4")).toBe(false);
    expect(isDateEgg("nope")).toBe(false);
  });
});
