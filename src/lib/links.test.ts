import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LinkTarget } from "./ipc";

// Backend et navigation simulés : seule la correspondance lien → page est testée ici.
const mocks = vi.hoisted(() => ({
  openLink: vi.fn<(text: string) => Promise<LinkTarget | null>>(),
  takeSharedLink: vi.fn<() => Promise<LinkTarget | null>>(),
  go: vi.fn(),
  pushToast: vi.fn(),
}));

vi.mock("./ipc", () => ({
  commands: { openLink: mocks.openLink, takeSharedLink: mocks.takeSharedLink },
}));
vi.mock("./stores/nav.svelte", () => ({ go: mocks.go }));
vi.mock("./stores/toasts.svelte", () => ({ pushToast: mocks.pushToast }));

const { openSharedLink, openTidalLink } = await import("./links");

beforeEach(() => {
  vi.clearAllMocks();
});

describe("openTidalLink", () => {
  it("ouvre l'album d'un titre, titre mis en évidence", async () => {
    mocks.openLink.mockResolvedValue({ kind: "album", id: 12, track: 34 });
    expect(await openTidalLink("https://tidal.com/track/34")).toBe(true);
    expect(mocks.go).toHaveBeenCalledWith({ name: "album", id: 12, track: 34 });
  });

  it("ouvre un album sans titre à mettre en évidence", async () => {
    mocks.openLink.mockResolvedValue({ kind: "album", id: 12, track: null });
    await openTidalLink("lien");
    expect(mocks.go).toHaveBeenCalledWith({ name: "album", id: 12, track: undefined });
  });

  it("ouvre artiste, playlist et mix", async () => {
    const cases: [LinkTarget, unknown][] = [
      [
        { kind: "artist", id: 5 },
        { name: "artist", id: 5 },
      ],
      [
        { kind: "playlist", uuid: "u-1" },
        { name: "playlist", uuid: "u-1" },
      ],
      [
        { kind: "mix", id: "m-1" },
        { name: "mix", id: "m-1" },
      ],
      // #328 : playlist d'un autre service → page d'import.
      [
        { kind: "import", link: "https://www.deezer.com/playlist/42" },
        { name: "import", link: "https://www.deezer.com/playlist/42" },
      ],
    ];
    for (const [target, route] of cases) {
      mocks.openLink.mockResolvedValue(target);
      await openTidalLink("lien");
      expect(mocks.go).toHaveBeenLastCalledWith(route);
    }
  });

  it("rend `false` sans lien reconnu, sans naviguer", async () => {
    mocks.openLink.mockResolvedValue(null);
    expect(await openTidalLink("du texte")).toBe(false);
    expect(mocks.go).not.toHaveBeenCalled();
  });

  it("affiche l'erreur du backend et considère le lien comme traité", async () => {
    mocks.openLink.mockRejectedValue("introuvable");
    expect(await openTidalLink("https://tidal.com/album/1")).toBe(true);
    expect(mocks.pushToast).toHaveBeenCalledWith({ level: "error", message: "introuvable" });
    expect(mocks.go).not.toHaveBeenCalled();
  });
});

describe("openSharedLink", () => {
  it("ouvre le lien en attente", async () => {
    mocks.takeSharedLink.mockResolvedValue({ kind: "artist", id: 9 });
    await openSharedLink();
    expect(mocks.go).toHaveBeenCalledWith({ name: "artist", id: 9 });
  });

  it("ouvre le fil d'une notification groupée des nouvelles sorties (#435)", async () => {
    mocks.takeSharedLink.mockResolvedValue({ kind: "feed" });
    await openSharedLink();
    expect(mocks.go).toHaveBeenCalledWith({ name: "feed" });
  });

  it("ignore l'absence de lien et les erreurs", async () => {
    mocks.takeSharedLink.mockResolvedValue(null);
    await openSharedLink();
    mocks.takeSharedLink.mockRejectedValue("erreur");
    await openSharedLink();
    expect(mocks.go).not.toHaveBeenCalled();
    expect(mocks.pushToast).not.toHaveBeenCalled();
  });
});
