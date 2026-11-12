import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TrackSort } from "./ipc";
import { entry } from "./testing/tracks";

// Mode conduite (#193) : une tuile lit sa source dans l'ordre de sa page, tri mémorisé compris.
const mocks = vi.hoisted(() => ({
  playlist: vi.fn(),
  libraryTracks: vi.fn(),
  mix: vi.fn(),
  saveForLater: vi.fn(),
  smartList: vi.fn(),
  playTracks: vi.fn(),
  sorts: new Map<string, TrackSort>(),
}));

vi.mock("./ipc", () => ({
  commands: {
    playlist: mocks.playlist,
    libraryTracks: mocks.libraryTracks,
    mix: mocks.mix,
    saveForLater: mocks.saveForLater,
    smartList: mocks.smartList,
    playTracks: mocks.playTracks,
  },
}));
vi.mock("./stores/ui.svelte", () => ({
  listSort: (list: string, fallback: TrackSort) => mocks.sorts.get(list) ?? fallback,
}));

const { playPin } = await import("./pins");

const UUID = "1b7a5f3e-2c4d-4e6f-8a9b-0c1d2e3f4a5b";
const old = entry({ title: "B", addedAt: 1 });
const recent = entry({ title: "A", addedAt: 2 });

beforeEach(() => {
  vi.clearAllMocks();
  mocks.sorts.clear();
  mocks.playlist.mockResolvedValue({ tracks: [old, recent] });
  mocks.libraryTracks.mockResolvedValue({ items: [recent, old] });
});

describe("playPin", () => {
  it("lit une playlist dans son ordre sans tri mémorisé", async () => {
    await playPin({ kind: "playlist", uuid: UUID, title: "Route" }, false);
    expect(mocks.playTracks).toHaveBeenCalledWith([old.id, recent.id], 0, false, {
      kind: "playlist",
      uuid: UUID,
      title: "Route",
    });
  });

  it("suit le tri mémorisé de la playlist", async () => {
    mocks.sorts.set(UUID, { key: "added", descending: true });
    await playPin({ kind: "playlist", uuid: UUID, title: "Route" }, true);
    expect(mocks.playTracks).toHaveBeenCalledWith([recent.id, old.id], 0, true, expect.anything());
  });

  it("suit le tri mémorisé des titres likés", async () => {
    mocks.sorts.set("likedTracks", { key: "title", descending: false });
    await playPin({ kind: "likedTracks" }, false);
    expect(mocks.playTracks).toHaveBeenCalledWith([recent.id, old.id], 0, false, { kind: "likedTracks" });
  });

  it("ne garde que les titres de « Plus tard »", async () => {
    mocks.saveForLater.mockResolvedValue({ items: [{ kind: "album", id: 9 }, { kind: "track", ...old }] });
    await playPin({ kind: "later" }, false);
    expect(mocks.playTracks).toHaveBeenCalledWith([old.id], 0, false, { kind: "later" });
  });

  it("lit une playlist intelligente dans l'ordre du backend (#383)", async () => {
    mocks.smartList.mockResolvedValue([recent, old]);
    await playPin({ kind: "smart", list: "onRepeat" }, false);
    expect(mocks.smartList).toHaveBeenCalledWith("onRepeat");
    expect(mocks.playTracks).toHaveBeenCalledWith([recent.id, old.id], 0, false, { kind: "smart", list: "onRepeat" });
  });

  it("refuse une source vide sans rien lire", async () => {
    mocks.playlist.mockResolvedValue({ tracks: [] });
    await expect(playPin({ kind: "playlist", uuid: UUID, title: "Vide" }, false)).rejects.toThrow();
    expect(mocks.playTracks).not.toHaveBeenCalled();
  });
});
