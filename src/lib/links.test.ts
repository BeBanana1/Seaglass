import { beforeEach, describe, expect, it, vi } from "vitest";
import type { LinkTarget } from "./ipc";

// Simulated backend and navigation: only the link → page mapping is tested here.
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
  it("opens a track's album, track highlighted", async () => {
    mocks.openLink.mockResolvedValue({ kind: "album", id: 12, track: 34 });
    expect(await openTidalLink("https://tidal.com/track/34")).toBe(true);
    expect(mocks.go).toHaveBeenCalledWith({ name: "album", id: 12, track: 34 });
  });

  it("opens an album with no track to highlight", async () => {
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
      // #328: playlist from another service → import page.
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

  it("returns `false` with no recognized link, without navigating", async () => {
    mocks.openLink.mockResolvedValue(null);
    expect(await openTidalLink("du texte")).toBe(false);
    expect(mocks.go).not.toHaveBeenCalled();
  });

  it("shows the backend's error and treats the link as handled", async () => {
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

  it("opens the feed of a grouped new-releases notification (#435)", async () => {
    mocks.takeSharedLink.mockResolvedValue({ kind: "feed" });
    await openSharedLink();
    expect(mocks.go).toHaveBeenCalledWith({ name: "feed" });
  });

  it("ignores a missing link and errors", async () => {
    mocks.takeSharedLink.mockResolvedValue(null);
    await openSharedLink();
    mocks.takeSharedLink.mockRejectedValue("erreur");
    await openSharedLink();
    expect(mocks.go).not.toHaveBeenCalled();
    expect(mocks.pushToast).not.toHaveBeenCalled();
  });
});
