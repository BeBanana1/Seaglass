import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "./i18n.svelte";
import { entry } from "./testing/tracks";
import {
  filterTracks,
  listTracks,
  normalize,
  playContext,
  sortTracks,
  type ListedTrack,
} from "./trackOrder";

const ids = (list: ListedTrack[]) => list.map(({ track }) => track.id);

afterEach(() => {
  i18n.lang = "fr";
});

describe("normalize", () => {
  it("retire accents et majuscules", () => {
    expect(normalize("Beyoncé")).toBe("beyonce");
    expect(normalize("ÉLÉPHANT Ça")).toBe("elephant ca");
  });
});

describe("filterTracks", () => {
  const a = entry({
    title: "Crazy in Love",
    artists: ["Beyoncé", "JAY-Z"],
    album: "Dangerously in Love",
  });
  const b = entry({
    title: "Halo",
    version: "Live",
    artists: ["Beyoncé"],
    album: "I Am... Sasha Fierce",
  });
  const c = entry({ title: "Hey Ya!", artists: ["OutKast"], album: "Speakerboxxx" });
  const list = listTracks([a, b, c]);

  it("garde tout sans requête", () => {
    expect(filterTracks(list, "  ")).toBe(list);
  });

  it("cherche dans le titre, la version, les artistes et l'album, sans accents", () => {
    expect(ids(filterTracks(list, "BEYONCE"))).toEqual([a.id, b.id]);
    expect(ids(filterTracks(list, "live"))).toEqual([b.id]);
    expect(ids(filterTracks(list, "speakerbox"))).toEqual([c.id]);
  });

  it("exige chaque mot, dans n'importe quel ordre", () => {
    expect(ids(filterTracks(list, "love jay"))).toEqual([a.id]);
    expect(ids(filterTracks(list, "halo outkast"))).toEqual([]);
  });

  it("garde la position d'origine", () => {
    expect(filterTracks(list, "hey")[0].position).toBe(3);
  });
});

describe("sortTracks", () => {
  it("revient à l'ordre d'origine, dans les deux sens", () => {
    const list = listTracks([entry(), entry(), entry()]);
    const desc = sortTracks(list, { key: "position", descending: true });
    expect(desc.map((t) => t.position)).toEqual([3, 2, 1]);
    expect(sortTracks(desc, { key: "position", descending: false }).map((t) => t.position)).toEqual(
      [1, 2, 3],
    );
  });

  it("ne modifie pas la liste reçue", () => {
    const list = listTracks([entry({ duration: 300 }), entry({ duration: 100 })]);
    const before = ids(list);
    sortTracks(list, { key: "duration", descending: false });
    expect(ids(list)).toEqual(before);
  });

  it("trie par titre sans tenir compte des accents ni de la casse, nombres dans l'ordre naturel", () => {
    const [e, a, track10, track2] = [
      entry({ title: "étoile" }),
      entry({ title: "Avion" }),
      entry({ title: "Track 10" }),
      entry({ title: "track 2" }),
    ];
    const sorted = sortTracks(listTracks([e, a, track10, track2]), {
      key: "title",
      descending: false,
    });
    expect(ids(sorted)).toEqual([a.id, e.id, track2.id, track10.id]);
  });

  it("est stable : à égalité, l'ordre d'origine, même en décroissant", () => {
    const [x, y, z] = [
      entry({ duration: 200 }),
      entry({ duration: 100 }),
      entry({ duration: 200 }),
    ];
    const list = listTracks([x, y, z]);
    expect(ids(sortTracks(list, { key: "duration", descending: false }))).toEqual([
      y.id,
      x.id,
      z.id,
    ]);
    expect(ids(sortTracks(list, { key: "duration", descending: true }))).toEqual([
      x.id,
      z.id,
      y.id,
    ]);
  });

  it("revient à l'ordre d'origine à égalité, même depuis une liste déjà triée autrement", () => {
    const [x, y, z] = [
      entry({ duration: 200 }),
      entry({ duration: 100 }),
      entry({ duration: 200 }),
    ];
    const reversed = sortTracks(listTracks([x, y, z]), { key: "position", descending: true });
    expect(ids(sortTracks(reversed, { key: "duration", descending: true }))).toEqual([
      x.id,
      z.id,
      y.id,
    ]);
  });

  it("trie par artistes et par album", () => {
    const [b, a] = [
      entry({ artists: ["Bjork"], album: "Zeta" }),
      entry({ artists: ["Air"], album: "Moon" }),
    ];
    const list = listTracks([b, a]);
    expect(ids(sortTracks(list, { key: "artist", descending: false }))).toEqual([a.id, b.id]);
    expect(ids(sortTracks(list, { key: "album", descending: true }))).toEqual([b.id, a.id]);
  });

  it("laisse en fin les titres sans date d'ajout, dans les deux sens", () => {
    const [none, old, recent, missing] = [
      entry({ addedAt: null }),
      entry({ addedAt: 1_000 }),
      entry({ addedAt: 5_000 }),
      // Fixtures anciennes : champ absent.
      { ...entry(), addedAt: undefined as unknown as null },
    ];
    const list = listTracks([none, old, recent, missing]);
    expect(ids(sortTracks(list, { key: "added", descending: false }))).toEqual([
      old.id,
      recent.id,
      none.id,
      missing.id,
    ]);
    expect(ids(sortTracks(list, { key: "added", descending: true }))).toEqual([
      recent.id,
      old.id,
      none.id,
      missing.id,
    ]);
  });
});

describe("playContext", () => {
  const [a, b, c] = [
    entry({ title: "Alpha" }),
    entry({ title: "Beta" }),
    entry({ title: "Gamma" }),
  ];

  it("met toute la liste triée en contexte et part du titre cliqué dans le filtre", () => {
    const sorted = sortTracks(listTracks([a, b, c]), { key: "position", descending: true });
    const shown = filterTracks(sorted, "beta");
    expect(playContext(sorted, shown, 0)).toEqual({ ids: [c.id, b.id, a.id], start: 1 });
  });

  it("distingue les doublons d'une playlist par leur position", () => {
    const sorted = listTracks([b, a, b]);
    const shown = filterTracks(sorted, "beta");
    expect(playContext(sorted, shown, 1)).toEqual({ ids: [b.id, a.id, b.id], start: 2 });
  });

  it("sans filtre, garde l'index cliqué", () => {
    const sorted = listTracks([a, b, c]);
    expect(playContext(sorted, filterTracks(sorted, ""), 2)).toEqual({
      ids: [a.id, b.id, c.id],
      start: 2,
    });
  });
});
