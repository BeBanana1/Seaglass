import { afterEach, describe, expect, it } from "vitest";
import { i18n } from "./i18n.svelte";
import { playedFrom } from "./playSource";

afterEach(() => {
  i18n.lang = "fr";
});

describe("playedFrom", () => {
  it("rien sans source", () => {
    expect(playedFrom(null)).toBeNull();
  });

  it("reprend le titre et la page d'un contenu", () => {
    expect(playedFrom({ kind: "album", id: 7, title: "Homogenic" })).toEqual({
      label: "Homogenic",
      route: { name: "album", id: 7 },
    });
    expect(playedFrom({ kind: "playlist", uuid: "abc", title: "Soirée" })?.route).toEqual({
      name: "playlist",
      uuid: "abc",
    });
    expect(playedFrom({ kind: "radio", seed: "track", id: 3, title: "Radio" })?.route).toEqual({
      name: "radio",
      seed: "track",
      id: 3,
    });
  });

  it("nomme les sources sans titre dans la langue courante", () => {
    expect(playedFrom({ kind: "artist", id: 1, name: "Björk" })?.label).toBe(
      "Titres populaires de Björk",
    );
    expect(playedFrom({ kind: "likedTracks" })).toEqual({
      label: "Titres likés",
      route: { name: "library", tab: "tracks" },
    });
    expect(playedFrom({ kind: "smart", list: "topMonth" })).toEqual({
      label: "Mes 50 du mois",
      route: { name: "smart", list: "topMonth" },
    });
    i18n.lang = "en";
    expect(playedFrom({ kind: "search", query: "jazz" })).toEqual({
      label: "Search “jazz”",
      route: { name: "search" },
    });
  });
});
