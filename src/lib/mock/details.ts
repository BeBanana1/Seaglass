import { allTracks, fx, params, type Handlers } from "./core";
import { hiresEdition, playback } from "./player";
import type { AlbumCreditGroup, ContributorPage, Lyrics, StreamFormat, TrackInfo } from "../ipc";

/** Couplets inventés, répartis sur toute la durée du titre (aucune vraie parole). */
const LYRIC_VERSES = [
  [
    "Le verre poli par la marée",
    "garde la couleur des étés",
    "on le ramasse sans y penser",
    "il brille encore au soleil levé",
  ],
  [
    "",
    "Refrain, les vagues reviennent",
    "et la lumière se souvient",
    "de tout ce qu'on n'a pas dit",
    "sur le sable, à minuit",
  ],
  [
    "Une bouteille il y a cent ans",
    "jetée au large par le vent",
    "",
    "elle revient en éclats lents",
    "plus douce qu'au commencement",
  ],
];

function mockLyrics(trackId: number): Lyrics | null {
  const mode = params.get("lyrics");
  if (mode === "none") return null;
  const lines = [...LYRIC_VERSES, LYRIC_VERSES[1]].flat();
  if (mode === "plain") {
    return { trackId, provider: "MUSIXMATCH", rtl: false, text: { kind: "plain", lines } };
  }
  const duration = allTracks.get(trackId)?.duration ?? 200;
  const step = (duration * 1000 - 8000) / lines.length;
  return {
    trackId,
    provider: "MUSIXMATCH",
    rtl: false,
    text: {
      kind: "synced",
      lines: lines.map((text, i) => ({ timeMs: Math.round(6000 + i * step), text })),
    },
  };
}

/** Infos factices (#3) : titre hi-res en 24/48, sinon 16/44,1 avec l'édition `?hiresedition=`. */
function mockTrackInfo(trackId: number): TrackInfo {
  const track = allTracks.get(trackId);
  if (!track) throw new Error(`titre ${trackId} introuvable`);
  const cd: StreamFormat = {
    quality: "LOSSLESS",
    codec: "flac",
    bitDepth: 16,
    sampleRate: 44100,
    bitrate: 893,
    hiresEdition: null,
  };
  const hires: StreamFormat = {
    ...cd,
    quality: "HI_RES_LOSSLESS",
    bitDepth: 24,
    sampleRate: 48000,
    bitrate: 2016,
  };
  return {
    track,
    tags: track.hires ? ["LOSSLESS", "HIRES_LOSSLESS"] : ["LOSSLESS"],
    stream: track.hires ? hires : cd,
    streamError: null,
    trackGain: -6.17,
    trackPeak: 0.997284,
    albumGain: -8.25,
    albumPeak: 0.997253,
    bpm: 147,
    key: track.hires ? "FSharp" : "Bb",
    keyScale: "MINOR",
    copyright: "℗ 2024 Label de démonstration",
    trackNumber: Math.max(1, fx.album.tracks.findIndex((t) => t.id === trackId) + 1),
    volumeNumber: 1,
    hiresEdition:
      !track.hires && hiresEdition !== null
        ? {
            track: {
              ...track,
              id: track.id + 1_000_000,
              hires: true,
              album: { ...track.album, title: hiresEdition.title },
            },
            stream: hires,
            played: playback.hiresUpgrade === "auto",
          }
        : null,
    credits: [
      {
        role: "Composer",
        contributors: track.artists.map((a) => ({ name: a.name, artistId: a.id })),
      },
      { role: "Lyricist", contributors: [{ name: "Ann Demo", artistId: null }] },
      {
        role: "Electric Guitar",
        contributors: [
          { name: track.artists[0]?.name ?? "Demo", artistId: track.artists[0]?.id ?? null },
          { name: "Bob Session", artistId: null },
        ],
      },
      { role: "Producer", contributors: [{ name: "Carla Studio", artistId: null }] },
      { role: "Mastering Engineer", contributors: [{ name: "Dan Lathe", artistId: null }] },
      { role: "Theremin Wrangler", contributors: [{ name: "Eve Unknown", artistId: null }] },
    ],
  };
}

function mockAlbumCredits(): AlbumCreditGroup[] {
  const ids = fx.album.tracks.map((t) => t.id);
  const main = fx.album.album.artists[0];
  return [
    {
      role: "Composer",
      contributors: [{ name: main?.name ?? "Demo", artistId: main?.id ?? null, tracks: [] }],
    },
    {
      role: "Drums",
      contributors: [
        { name: "Dee Kit", artistId: null, tracks: ids.filter((_, i) => i % 2 === 0) },
        { name: "Ed Brush", artistId: null, tracks: ids.slice(1, 4) },
      ],
    },
    { role: "Producer", contributors: [{ name: "Carla Studio", artistId: null, tracks: [] }] },
    {
      role: "Mastering Engineer",
      contributors: [{ name: "Dan Lathe", artistId: null, tracks: ids.slice(-1) }],
    },
  ];
}

/** Titres d'un contributeur (#429) : lots de 8 sur les titres des fixtures, pour voir « Plus de titres ». */
function mockContributor(offset: number): ContributorPage {
  const pool = [...fx.album.tracks, ...fx.search.tracks];
  const roles = [["Producer", "Composer"], ["Mixing Engineer"], ["Bass"], ["RecordingArranger"]];
  const batch = pool.slice(offset, offset + 8);
  const next = offset + 8;
  return {
    artist: fx.artist.artist,
    total: pool.length,
    tracks: batch.map((track, i) => ({ track, roles: roles[(offset + i) % roles.length] })),
    nextOffset: next < pool.length ? next : null,
  };
}

export const detailsHandlers: Handlers = {
  lyrics: ({ trackId }) => mockLyrics(Number(trackId)),
  track_info: ({ trackId }) => mockTrackInfo(Number(trackId)),
  album_credits: () => mockAlbumCredits(),
  contributor: ({ offset }) => mockContributor(Number(offset)),
};
