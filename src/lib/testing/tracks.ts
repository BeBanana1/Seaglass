// Données des tests du frontend (#264) : titres minimaux, champs utiles seulement.

import type { TrackEntry } from "../ipc";

let nextId = 1;

/** Titre de liste ; les champs non donnés prennent une valeur neutre. */
export function entry(
  fields: Partial<Omit<TrackEntry, "artists" | "album">> & { artists?: string[]; album?: string } = {},
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
    addedAt: null,
    ...rest,
    artists: artists.map((name, i) => ({ id: i + 1, name })),
    album: { id, title: album, coverUrl: null, color: null },
  };
}
