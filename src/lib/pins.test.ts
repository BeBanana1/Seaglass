import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TrackSort } from "./ipc";
import { entry } from "./testing/tracks";

// Driving mode (#193): a tile reads its source in its page's order, remembered sort included.
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
  it("plays a playlist in its order with no remembered sort", async () => {
    await playPin({ kind: "playlist", uuid: UUID, title: "Route" }, false);
    expect(mocks.playTracks).toHaveBeenCalledWith([old.id, recent.id], 0, false, {
      kind: "playlist",
      uuid: UUID,
      title: "Route",
    });
  });

  it("follows the playlist's remembered sort", async () => {
    mocks.sorts.set(UUID, { key: "added", descending: true });
    await playPin({ kind: "playlist", uuid: UUID, title: "Route" }, true);
    expect(mocks.playTracks).toHaveBeenCalledWith([recent.id, old.id], 0, true, expect.anything());
  });

  it("follows the remembered sort of the liked tracks", async () => {
    mocks.sorts.set("likedTracks", { key: "title", descending: false });
    await playPin({ kind: "likedTracks" }, false);
    expect(mocks.playTracks).toHaveBeenCalledWith([recent.id, old.id], 0, false, {
      kind: "likedTracks",
    });
  });

  it('keeps only the tracks of "Save for later"', async () => {
    mocks.saveForLater.mockResolvedValue({
      items: [
        { kind: "album", id: 9 },
        { kind: "track", ...old },
      ],
    });
    await playPin({ kind: "later" }, false);
    expect(mocks.playTracks).toHaveBeenCalledWith([old.id], 0, false, { kind: "later" });
  });

  it("plays a smart playlist in the backend's order (#383)", async () => {
    mocks.smartList.mockResolvedValue([recent, old]);
    await playPin({ kind: "smart", list: "onRepeat" }, false);
    expect(mocks.smartList).toHaveBeenCalledWith("onRepeat");
    expect(mocks.playTracks).toHaveBeenCalledWith([recent.id, old.id], 0, false, {
      kind: "smart",
      list: "onRepeat",
    });
  });

  it("refuses an empty source without playing anything", async () => {
    mocks.playlist.mockResolvedValue({ tracks: [] });
    await expect(playPin({ kind: "playlist", uuid: UUID, title: "Vide" }, false)).rejects.toThrow();
    expect(mocks.playTracks).not.toHaveBeenCalled();
  });
});
