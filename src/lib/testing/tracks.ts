// Frontend test data (#264): minimal tracks, only the useful fields.

import type { TrackEntry } from "../ipc";

let nextId = 1;

/** List track; the fields not given take a neutral value. */
export function entry(
  fields: Partial<Omit<TrackEntry, "artists" | "album">> & {
    artists?: string[];
    album?: string;
  } = {},
): TrackEntry {
  const { artists = ["Artiste"], album = "Album", ...rest } = fields;
  const id = nextId++;
  return {
    id,
    title: `Titre ${id}`,
    version: null,
    duration: 180,
    explicit: false,
    ai: false,
    hires: false,
    isrc: null,
    addedAt: null,
    ...rest,
    artists: artists.map((name, i) => ({ id: i + 1, name })),
    album: {
      id,
      title: album,
      coverUrl: null,
      coverSmallUrl: null,
      coverLargeUrl: null,
      color: null,
    },
  };
}
