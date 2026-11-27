// Typed wrappers for the Tauri commands and events.
// Types generated from Rust by ts-rs live in `bindings/` (#44, `cargo test` writes them) and are
// re-exported here; the others are still copied by hand from Rust (tidal/models.rs,
// player/state.rs…) until their group moves to `bindings/`.

import { invoke, listen, type UnlistenFn } from "./transport";
import type { AlbumRef } from "./bindings/AlbumRef";
import type { ArtistRef } from "./bindings/ArtistRef";
import type { AudioParams } from "./bindings/AudioParams";
import type { PlaySource } from "./bindings/PlaySource";
import type { PlayerState } from "./bindings/PlayerState";
import type { QueuePart } from "./bindings/QueuePart";
import type { QueueView } from "./bindings/QueueView";
import type { RadioSeed } from "./bindings/RadioSeed";
import type { Repeat } from "./bindings/Repeat";
import type { SmartList } from "./bindings/SmartList";
import type { StreamFormat } from "./bindings/StreamFormat";
import type { Track } from "./bindings/Track";
import type { TrackId } from "./bindings/TrackId";
import type { Volume } from "./bindings/Volume";

export type { AlbumRef } from "./bindings/AlbumRef";
export type { ArtistRef } from "./bindings/ArtistRef";
export type { AudioParams } from "./bindings/AudioParams";
export type { AudioQuality } from "./bindings/AudioQuality";
export type { EditionAlbum } from "./bindings/EditionAlbum";
export type { MpvIssue } from "./bindings/MpvIssue";
export type { OutputDevice } from "./bindings/OutputDevice";
export type { PlaySource } from "./bindings/PlaySource";
export type { PlayerState } from "./bindings/PlayerState";
export type { QueuePart } from "./bindings/QueuePart";
export type { QueueView } from "./bindings/QueueView";
export type { RadioSeed } from "./bindings/RadioSeed";
export type { Repeat } from "./bindings/Repeat";
export type { SleepState } from "./bindings/SleepState";
export type { SmartList } from "./bindings/SmartList";
export type { StreamFormat } from "./bindings/StreamFormat";
export type { Track } from "./bindings/Track";
export type { TrackId } from "./bindings/TrackId";
export type { Volume } from "./bindings/Volume";

/** Result of a settings import (#352). */
/** #330: library export finished (folder already opened). */
export interface LibraryExportReport {
  dir: string;
  tracks: number;
  albums: number;
  artists: number;
  playlists: number;
  /** Titles of the unreadable playlists, left out. */
  failed: string[];
  /** At least one list is cut at the loading cap. */
  truncated: boolean;
}

/** #330: playlists read out of the total. */
export interface LibraryExportProgress {
  done: number;
  total: number;
}

export interface ImportReport {
  applied: string[];
  ignored: string[];
}

/** A list entry (playlist or favorites) with the date it was added to that list. */
export interface TrackEntry extends Track {
  /** Milliseconds since the Unix epoch (UTC); `null` if the API does not give it. */
  addedAt: number | null;
}

/** One library category, from newest to oldest (#91). */
export interface LibraryList<T> {
  items: T[];
  /** Cut at the loading cap: only the most recent ones are there. */
  truncated: boolean;
  /** Count announced by TIDAL (unavailable ones included), if known. */
  total: number | null;
}

/** `commands::project::ProjectPage`: project page opened in the browser (#188, #517). */
export type ProjectPage = "website" | "repo" | "newIssue" | "clientId";

/** Detailed info of a track (#3), requested when the panel opens. */
export interface TrackInfo {
  track: Track;
  /** `LOSSLESS`, `HIRES_LOSSLESS`, `DOLBY_ATMOS`… */
  tags: string[];
  /** Format served at maximum quality; `null` if refused (see `streamError`). */
  stream: StreamFormat | null;
  streamError: string | null;
  /** ReplayGain in dB and peaks (full scale = 1), never applied by Seaglass. */
  trackGain: number | null;
  trackPeak: number | null;
  albumGain: number | null;
  albumPeak: number | null;
  bpm: number | null;
  /** Raw key from the API (`B`, `FSharp`, `Bb`…) and mode (`MAJOR`, `MINOR`). */
  key: string | null;
  keyScale: string | null;
  copyright: string | null;
  trackNumber: number | null;
  volumeNumber: number | null;
  /** Other hi-res edition of the same recording, if the track is not hi-res. */
  hiresEdition: HiresEdition | null;
  /** Credits grouped by role, sorted by the backend; empty if TIDAL has none (#84). */
  credits: CreditGroup[];
}

export interface CreditGroup {
  /** Free text in English (`Composer`, `Bass guitar`…), translated by `creditRole`. */
  role: string;
  contributors: Contributor[];
}

export interface Contributor {
  name: string;
  /** TIDAL artist (artist page), if there is one. */
  artistId: number | null;
}

/** Page to open for a pasted TIDAL link (#203). */
export type LinkTarget =
  | { kind: "album"; id: number; track: TrackId | null }
  | { kind: "artist"; id: number }
  | { kind: "playlist"; uuid: string }
  | { kind: "mix"; id: string }
  /** Playlist from another service to import (#328): clean address, re-read by `importPlaylist`. */
  | { kind: "import"; link: string }
  /** News feed: grouped notification of new releases (#435). */
  | { kind: "feed" };

/** Import of a playlist from another service (#328): a source track and its match. */
export interface ImportItem {
  title: string;
  artist: string | null;
  /** `null`: nothing similar on TIDAL. */
  track: Track | null;
  /** Title, artist and duration match (or same ISRC); otherwise to be checked. */
  sure: boolean;
}

export type ImportService = "deezer" | "spotify" | "apple";

export interface ImportPreview {
  name: string;
  service: ImportService;
  /** Tracks of the playlist at the service. */
  total: number;
  /** Tracks read: fewer than `total` if the service only shows part of them (Spotify: 100). */
  read: number;
  items: ImportItem[];
}

export interface ImportProgress {
  done: number;
  total: number;
}

/** Credits of a whole album (#206). */
export interface AlbumCreditGroup {
  role: string;
  contributors: AlbumContributor[];
}

/** Tracks where a person is credited (#429), in batches. */
export interface ContributorPage {
  artist: ArtistSummary;
  /** Number of tracks announced by the service. */
  total: number;
  tracks: Contribution[];
  /** To pass back to `contributor` for the next batch; `null` at the end. */
  nextOffset: number | null;
}

export interface Contribution {
  track: Track;
  /** English roles from the API (`Producer`, `Composer`…), translated by `creditRole`. */
  roles: string[];
}

export interface AlbumContributor {
  name: string;
  artistId: number | null;
  /** Tracks where they appear, in album order; empty = all. */
  tracks: TrackId[];
}

export interface HiresEdition {
  track: Track;
  stream: StreamFormat | null;
  /** Seaglass plays it instead of the track (setting on and better stream). */
  played: boolean;
}

/** WASAPI output device (#79): mpv identifier (`wasapi/{guid}`) and Windows name. */
export interface AudioDevice {
  id: string;
  name: string;
}

/** Devices known to mpv (`audio_devices` command, `audio-devices` event). */
export interface AudioDevices {
  devices: AudioDevice[];
  /** Identifier of the Windows default device, if known. */
  systemDefault: string | null;
}

/** Blind test (#434): free typing or four choices. */
export type BlindMode = "typing" | "choices";

export type BlindAnswer =
  { kind: "text"; text: string } | { kind: "choice"; index: number } | { kind: "pass" };

export interface BlindChoice {
  title: string;
  artist: string;
}

/** Player of a game (#441): the host (`id` 0) or a Jam guest. */
export interface BlindPlayer {
  id: number;
  /** First name given in the Jam; `null`: "Guest <id>". */
  name: string | null;
  host: boolean;
  score: number;
  streak: number;
  bestStreak: number;
  /** Has answered the current round. */
  answered: boolean;
}

export interface BlindPlayerResult {
  id: number;
  name: string | null;
  host: boolean;
  answered: boolean;
  title: boolean;
  artist: boolean;
  points: number;
  /** First to find the title (bonus included in `points`). */
  fastest: boolean;
}

export interface BlindParty {
  jam: boolean;
  hostPlays: boolean;
}

export interface BlindRoundResult {
  track: Track;
  title: boolean;
  artist: boolean;
  points: number;
  answer: string | null;
  chosen: number | null;
  correct: number | null;
  /** All players, from most to fewest points (#441). */
  players: BlindPlayerResult[];
}

/** Game held by the backend (`blind-test` event); the round's track is only there once
 *  revealed, in `results`. */
export interface BlindTestView {
  title: string;
  mode: BlindMode;
  phase: "guess" | "revealed" | "finished";
  /** Current round, starting at 1. */
  round: number;
  rounds: number;
  score: number;
  maxScore: number;
  streak: number;
  bestStreak: number;
  clipStart: number;
  clipLength: number;
  choices: BlindChoice[];
  results: BlindRoundResult[];
  party: BlindParty;
  /** Ranking; the host alone in solo. */
  players: BlindPlayer[];
  /** Automatic reveal planned (ms since the epoch), with several players. */
  revealAt: number | null;
  /** The clip is really playing (#445): before that, the engine is still loading the track. */
  clipPlaying: boolean;
}

export type SleepRequest =
  { kind: "minutes"; minutes: number } | { kind: "endOfTrack" } | { kind: "endOfContext" };

export interface AlbumSummary {
  id: number;
  title: string;
  artists: ArtistRef[];
  coverUrl: string | null;
  /** 640×640 (#89). */
  coverLargeUrl?: string | null;
  color: string | null;
  /** `YYYY-MM-DD`. */
  releaseDate: string | null;
  numberOfTracks: number | null;
  /** `ALBUM`, `EP` or `SINGLE`. */
  albumType: string | null;
  explicit: boolean;
  /** AI-generated according to the service (#369). */
  ai: boolean;
  hires: boolean;
}

export interface ArtistSummary {
  id: number;
  name: string;
  pictureUrl: string | null;
}

export interface PlaylistSummary {
  uuid: string;
  title: string;
  description: string | null;
  imageUrl: string | null;
  numberOfTracks: number | null;
  duration: number | null;
  editorial: boolean;
  mine: boolean;
}

/** Playlist folder of the library (#331), a single level. */
export interface PlaylistFolder {
  id: string;
  name: string;
  /** Uuids of the stored playlists (yours and the ones you follow). */
  playlists: string[];
}

/** Mix generated by the service (My Mix, Daily Discovery, artist mix…). */
export interface MixSummary {
  id: string;
  title: string;
  subtitle: string | null;
  imageUrl: string | null;
  /** `DAILY_MIX`, `DISCOVERY_MIX`, `ARTIST_MIX`, `TRACK_MIX`… */
  mixType: string | null;
}

export type MediaItem =
  | ({ kind: "album" } & AlbumSummary)
  | ({ kind: "artist" } & ArtistSummary)
  | ({ kind: "playlist" } & PlaylistSummary)
  | ({ kind: "mix" } & MixSummary);

/** "Save for later" item (#196): a track with its added date, or a card. */
export type LaterItem =
  | ({ kind: "track" } & TrackEntry)
  | ({ kind: "album" } & AlbumSummary)
  | ({ kind: "artist" } & ArtistSummary)
  | ({ kind: "playlist" } & PlaylistSummary);

/** What can be added to favorites (#4, #81). */
export type LikeTarget =
  | { kind: "track"; id: TrackId }
  | { kind: "album"; id: number }
  | { kind: "artist"; id: number }
  | { kind: "playlist"; uuid: string }
  | { kind: "mix"; id: string };

/** What can be blocked (#334): suggested neither by mixes nor by radios. */
export type BlockTarget = { kind: "track"; id: TrackId } | { kind: "artist"; id: number };

/** Blocked tracks and artists, from newest to oldest. */
export interface Blocked {
  tracks: Track[];
  artists: ArtistSummary[];
}

/** All favorites, as identifiers (`likes` event). */
/** Position in a playlist (0 = first) and track expected at that position (#332). */
export interface PlaylistItem {
  index: number;
  trackId: TrackId;
}

export interface LikedIds {
  tracks: TrackId[];
  albums: number[];
  artists: number[];
  /** Followed playlists. */
  playlists: string[];
  /** Saved mixes (#204). */
  mixes: string[];
}

export interface HomeRow {
  /** Stable identifier of the section (`moduleId` of the API), independent of the title. */
  key: string;
  title: string;
  items: MediaItem[];
}

/** Home row in the chosen order; hidden ones are only shown while customizing. */
export interface HomeSection extends HomeRow {
  hidden: boolean;
}

/** First page of the home; `more`: the rest comes from `homeRest` (#207). */
export interface HomeFeed {
  sections: HomeSection[];
  more: boolean;
}

/** News feed item: release by a followed artist (album) or monthly mix (mix). */
export interface FeedItem {
  /** Release date, in UTC milliseconds. */
  occurredAt: number | null;
  /** Already seen in the app (local state, #415; on first read, taken from TIDAL). */
  seen: boolean;
  item: MediaItem;
}

/** Saved home layout (keys of `HomeRow`). */
export interface HomeLayout {
  order: string[];
  hidden: string[];
  /** "Featured" banner hidden (#185). */
  featuredHidden: boolean;
}

export interface AlbumPage {
  album: AlbumSummary;
  coverLargeUrl: string | null;
  duration: number | null;
  copyright: string | null;
  tracks: Track[];
}

export interface ArtistPage {
  artist: ArtistSummary;
  topTracks: Track[];
  albums: AlbumSummary[];
  singles: AlbumSummary[];
}

/** Local listening history (#202). */
export interface PlayHistory {
  /** Albums listened to, from newest to oldest. */
  recent: AlbumSummary[];
  /** Most listened albums over the last 30 days (at least two listens). */
  top: AlbumSummary[];
}

/** Period of the listening statistics (#326): sliding windows of 7, 30, 365 days. */
export type StatsPeriod = "week" | "month" | "year" | "all";

export interface Counted<T> {
  item: T;
  plays: number;
}

/** Local listening statistics (#326), specific to this device. */
export interface PlayStats {
  plays: number;
  /** Approximate listening time (sum of the durations of the tracks listened), in seconds. */
  seconds: number;
  /** Listens of unknown duration (before 0.5.33), absent from `seconds`. */
  unknown: number;
  tracks: Counted<Track>[];
  albums: Counted<AlbumSummary>[];
  /** Picture requested once from TIDAL, then kept with the history. */
  artists: Counted<ArtistSummary>[];
}

/** Period of the recap (#417): calendar month, or whole year without `month`. */
export interface RecapPeriod {
  year: number;
  month: number | null;
}

/** Recap of a month or a year (#417), computed over this device's history. */
export interface Recap {
  plays: number;
  seconds: number;
  unknown: number;
  /** Days with at least one listen. */
  days: number;
  tracks: Counted<Track>[];
  albums: Counted<AlbumSummary>[];
  artists: Counted<ArtistSummary>[];
  /** Artists with no listen at all before the period. */
  newArtists: Counted<ArtistSummary>[];
  newArtistCount: number;
  newTrackCount: number;
  /** Listens per weekday, Monday first. */
  weekdays: number[];
  /** Listens per local hour, from 0 to 23. */
  hours: number[];
  /** 0 = Monday. */
  topWeekday: number | null;
  topHour: number | null;
  /** Plays per month for a year, per day for a month (#521). */
  timeline: number[];
  /** Plays by weekday (Monday first) then local hour: 7 rows of 24. */
  grid: number[][];
  /** Longest run of consecutive listening days. */
  streak: number;
  /** Track played the most times in a single day (at least twice). */
  topDay: { track: Track; plays: number; date: string } | null;
  firstTrack: Track | null;
  /** Audio side (#524), over the listens recorded with their format; null when none has one. */
  audio: AudioRecap | null;
}

export interface AudioRecap {
  /** Listens with a known format: the shares below are out of this number. */
  known: number;
  hires: number;
  lossless: number;
  lossy: number;
  bitPerfect: number;
  bitPerfectSeconds: number;
  formats: PlayedFormat[];
}

export interface PlayedFormat {
  codec: string;
  bitDepth: number | null;
  sampleRate: number | null;
  plays: number;
}

export const SMART_LISTS: SmartList[] = ["topMonth", "onRepeat", "forgotten", "neverPlayed"];

/** Key of the "Listened in Seaglass" home section. */
export const PLAY_HISTORY_KEY = "SEAGLASS_RECENT";

/** Biography segment: text or internal link (#205). */
export type BioPart =
  | { kind: "text"; text: string }
  | { kind: "artist"; text: string; id: number }
  | { kind: "album"; text: string; id: number };

/** Bottom of the album page (#350), loaded after it: each list can be empty. */
export interface AlbumRelated {
  versions: AlbumSummary[];
  byArtist: AlbumSummary[];
}

/** Extras of the artist page (#205), loaded after it. */
export interface ArtistExtra {
  bio: { source: string | null; paragraphs: BioPart[][] } | null;
  similar: ArtistSummary[];
  appearsOn: AlbumSummary[];
}

export interface PlaylistPage {
  playlist: PlaylistSummary;
  /** In playlist order. */
  tracks: TrackEntry[];
  /** Version of the playlist read (#152): hand back to `playlist` to know if it changed. */
  revision: string | null;
}

/** Change proposed by "Clean up" (#382); `index` = position in the list read,
 *  `op` = what accepting it does (sent back as is). */
export type CleanupItem = (
  | {
      kind: "duplicate";
      index: number;
      track: Track;
      keptIndex: number;
      kept: Track;
      sameTrack: boolean;
    }
  | { kind: "unavailable"; index: number; track: Track; replacement: Track | null }
) & {
  op: ListOp;
  /** Checked by default (not an unavailable track without replacement: its removal is final). */
  suggested: boolean;
};

export interface CleanupReport {
  items: CleanupItem[];
  /** Tracks read. */
  scanned: number;
}

/** Change to a list (#382); favorites ignore `index`. */
export type ListOp =
  | { op: "remove"; index: number; trackId: TrackId }
  | { op: "insert"; index: number; trackId: TrackId }
  | { op: "replace"; index: number; trackId: TrackId; with: TrackId };

export interface ListOpsResult {
  applied: number;
  /** Tracks the service refuses to add back (removed from the catalog). */
  skipped: number;
  /** What it takes to undo, to hand back as is. */
  undo: ListOp[];
  error: string | null;
}

/** Lyrics (`tidal/models.rs`: `Lyrics`). */
export interface Lyrics {
  trackId: TrackId;
  /** Provider to credit ("MUSIXMATCH"). */
  provider: string | null;
  /** Right-to-left writing. */
  rtl: boolean;
  text: { kind: "synced"; lines: LyricLine[] } | { kind: "plain"; lines: string[] };
}

export interface LyricLine {
  /** Start of the line, in milliseconds; empty text = instrumental passage. */
  timeMs: number;
  text: string;
}

export interface MixPage {
  mix: MixSummary;
  /** 640×640 image for the header. */
  imageLargeUrl: string | null;
  tracks: Track[];
}

/** Radio of a track or an artist. */
export interface RadioPage {
  seed: RadioSeed;
  id: number;
  /** Title of the starting track or name of the starting artist. */
  title: string;
  imageUrl: string | null;
  tracks: Track[];
}

export interface SearchResults {
  tracks: Track[];
  albums: AlbumSummary[];
  artists: ArtistSummary[];
  playlists: PlaylistSummary[];
}

export type Theme =
  | "seaglass"
  | "studio"
  | "atelier"
  | "acidwave"
  | "surreal"
  | "solarpunk"
  | "cyberpunk"
  | "neumorph"
  | "neumorph-dark"
  | "terminal"
  | "riso"
  | "bauhaus"
  | "aero"
  | "alloy"
  | "alloy-dark"
  | "midnight"
  | "estampe"
  | "artdeco"
  | "cassette"
  | "seance"
  | "onboard"
  | "noir"
  | "hotel"
  | "desert"
  | "generique";

/** Sidebar shortcut (desktop) or driving mode tile (Android, #193); the title of a playlist
 *  or a mix is kept to display it without a request. */
export type Pin =
  | { kind: "playlist"; uuid: string; title: string }
  | { kind: "mix"; id: string; title: string }
  | { kind: "likedTracks" }
  | { kind: "later" }
  | { kind: "smart"; list: SmartList };

/** Sort criteria of a track list (#1, #2). */
export type SortKey = "position" | "added" | "title" | "artist" | "album" | "duration";

export interface TrackSort {
  key: SortKey;
  descending: boolean;
}

/** Remembered sort of a list (#93): playlist uuid, or `likedTracks`. */
export interface ListSort extends TrackSort {
  list: string;
}

/** Version of the log (#340), entries already filtered for the platform. */
export interface Release {
  version: string;
  /** `YYYY-MM-DD` for a published version. */
  published: string | null;
  entries: string[];
}

export interface UiSettings {
  theme: Theme;
  home: HomeLayout;
  pins: Pin[];
  /** `#rrggbb` accent color of each theme (#250); theme absent: the one from its CSS. */
  accents: Partial<Record<Theme, string>>;
  /** Favorite theme families (#390), at the top of the gallery; see `lib/themes.ts`. */
  favoriteThemes: Theme[];
  /** Chosen language (#7); `system` follows the Windows display language. */
  language: LanguagePref;
  /** Closing the window: quit, or minimize to the notification area (#83). */
  closeAction: CloseAction;
  /** Discord presence (#215), off by default. */
  discord: boolean;
  /** Remembered sorts, most recent first (#93). */
  sorts: ListSort[];
  /** Interface size in percent (#337), one value of `ZOOM_LEVELS`. */
  zoom: number;
  /** Interface animations (#351). */
  motion: Motion;
  /** Interface winks (#399), on by default; see `lib/eggs/`. */
  easterEggs: boolean;
  /** Content hidden in what the service offers (#368); filtered on the Rust side. */
  content: ContentFilter;
  /** Global keyboard shortcuts (#413, desktop). */
  globalShortcuts: GlobalShortcuts;
  /** Periodic check for updates (#452), on by default. */
  autoUpdates: boolean;
  /** Jam (#452): when off, it disappears from the UI. */
  jam: boolean;
  /** Notification of new releases by followed artists (#435), off by default. */
  releaseNotifications: boolean;
}

/** Sending listens to ListenBrainz (#431). The token is never sent back to the UI. */
export interface ListenBrainzStatus {
  /** Connected account; `null`: sending disabled. */
  user: string | null;
  /** ListenBrainz refused the token on a send: a new one must be pasted. */
  refused: boolean;
  /** Listens not sent yet. */
  pending: number;
}

/** Relay of the online Jam and of the transfers (#452). The key is never sent back. */
export interface RelayView {
  enabled: boolean;
  /** Chosen address; `null`: the compiled one (`defaultUrl`). */
  url: string | null;
  defaultUrl: string | null;
  /** A key specific to this relay is saved. */
  customKey: boolean;
  /** Address and key known: the relay is usable. */
  usable: boolean;
}

/** Empty `url`: compiled relay. `key` absent: unchanged; empty: compiled key. */
export interface RelayChange {
  enabled: boolean;
  url: string | null;
  key?: string | null;
}

/** Action of a global shortcut (#413). */
export type GlobalAction =
  "playPause" | "next" | "previous" | "volumeUp" | "volumeDown" | "like" | "showWindow";

/** Combinations written `Ctrl+Alt+Space` (modifiers then `KeyboardEvent.code`); action
 *  without a combination: absent. */
export interface GlobalShortcuts {
  enabled: boolean;
  bindings: Partial<Record<GlobalAction, string>>;
}

export interface GlobalShortcutsStatus {
  /** The system offers global shortcuts (false on Android, or without an X server). */
  available: boolean;
  /** Combinations not registered: taken by another app, or unknown key. */
  failed: GlobalAction[];
}

/** What the backend removes from the offered lists (not from favorites or own playlists). */
export interface ContentFilter {
  hideExplicit: boolean;
  /** Hidden by default (#369). */
  hideAi: boolean;
}

export type CloseAction = "quit" | "tray";

/** `system` follows the system's "reduce animations". */
export type Motion = "system" | "reduced" | "full";

/** Effective language of the app, decided by the backend. */
export type Lang = "fr" | "en";

/** Platform of the app, decided at compile time on the Rust side. */
export type Platform = "desktop" | "android";

export type LanguagePref = "system" | Lang;

/** Size of the mpv cache in MiB: only these values are accepted by the backend. */
export type CacheSize = 32 | 64 | 128 | 256;

export type HiresUpgrade = "auto" | "ask" | "off";

/**
 * Playback quality on Android (#325, #366), TIDAL tiers: AAC 96 and 320 kbit/s, FLAC
 * 16/44.1, hi-res FLAC up to 24/192.
 */
export type MobileQuality = "low" | "high" | "lossless" | "max";

/** Volume normalization through ReplayGain (#82). */
export type Normalization = "off" | "track" | "album";

/** Parametric equalizer (#428). Bounds checked on the Rust side (`player::eq`). */
export type EqFilterKind = "peaking" | "lowShelf" | "highShelf";

export interface EqFilter {
  kind: EqFilterKind;
  /** Hz, 20 to 20,000. */
  freq: number;
  /** dB, ±20. */
  gain: number;
  /** 0.1 to 10. */
  q: number;
}

export interface EqProfile {
  id: number;
  name: string;
  /** dB; `null` = automatic (compensates the strongest positive gain). */
  preamp: number | null;
  filters: EqFilter[];
}

export interface EqSettings {
  /** Off by default. */
  enabled: boolean;
  active: number | null;
  profiles: EqProfile[];
}

/** Playback settings, stored apart from the UI settings. */
export interface PlaybackSettings {
  cacheSize: CacheSize;
  /** Hi-res edition of a track that has none (same recording): play it instead,
   *  offer it (#59) or ignore it. */
  hiresUpgrade: HiresUpgrade;
  /** Off by default: the only mode that leaves the signal intact. */
  normalization: Normalization;
  equalizer: EqSettings;
  /** Chosen output device (#79); `null` = system default device.
   *  Kept even when unplugged: the player falls back to the default then returns to it. */
  audioDevice: AudioDevice | null;
  /** WASAPI exclusive mode (#245), off by default: normalization suspended, other
   *  apps have no sound during playback. No effect on Android. */
  exclusive: boolean;
  /** End of queue: continue with the radio of the last track (#78). */
  autoRadio: boolean;
  /** Desktop: pause when the output device in use is unplugged (#348). On
   *  by default. */
  pauseOnUnplug: boolean;
  /** Desktop: resume playback when the app launches (#353). Off by default. */
  resumeOnLaunch: boolean;
  /** Desktop: volume, normalization and exclusive mode kept per output device
   *  (#416). Off by default. */
  deviceProfiles: boolean;
  /** Android: requested quality (#325), Lossless by default. No effect on desktop. */
  /** Android (#365): unmetered network (Wi-Fi) and metered (mobile data). */
  wifiQuality: MobileQuality;
  cellularQuality: MobileQuality;
  /** Android (#365): cap when the battery is low. */
  lowBatteryQuality: boolean;
}

/** Android (#492): quality lowered by the battery right now. */
export interface QualityCap {
  /** Quality requested for now. */
  quality: MobileQuality;
  /** The one from the setting for the current network. */
  chosen: MobileQuality;
  reason: "powerSave" | "batteryLow";
  /** Level in %, if known. */
  battery: number | null;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
  /** Android only: size of the APK in bytes, offered before it is downloaded. */
  size?: number;
  /** Desktop: installer being downloaded, `update-ready` will follow (#187). */
  downloading?: boolean;
}

export interface SessionInfo {
  userId: number;
  countryCode: string;
}

export interface Toast {
  level: "error" | "info";
  message: string;
  /** Player refusal (wink #399), which the UI can dress up according to the theme. */
  kind?: "unplayable" | "stopped";
}

/** Jam (#25): online through the relay, or on the local network. */
export type JamMode = "online" | "local";
export type JamConnection = "connecting" | "online" | "reconnecting" | "failed";

/** Remote control server (#336) running, desktop only: remote control and / or
 *  playback transfers (#414). */
export interface RemoteStatus {
  /** Remote control page and commands served. */
  remote: boolean;
  /** Playback transfers accepted from the Android app. */
  handoff: boolean;
  url: string;
  /** Owner code, 12 characters. */
  code: string;
  /** False if no local network address was found. */
  lan: boolean;
  qrSvg: string | null;
  /** End of the last pairing window (ms since the epoch, #62), `null` before any. */
  pairingUntil: number | null;
  /** A relay is set up: handoffs also work away from home, "Link a phone" is offered. */
  relay: boolean;
}

/** Computer linked for playback transfers (#414), phone side. */
export interface LinkedComputer {
  /** `192.168.1.105:47801`. */
  host: string;
  /** Last successful sync of the listening history (ms, #430). */
  syncedAt: number | null;
  /** Reachable also away from the local network, through the relay (#430). */
  online: boolean;
}

/** Jam (#25): address and code to give to the guests. */
export interface JamStatus {
  mode: JamMode;
  /** `null` until the relay has answered. */
  url: string | null;
  code: string;
  /** Local network: false if no network address was found (`127.0.0.1`). */
  lan: boolean;
  /** QR code of the address with the code, as black-on-white SVG (generated by the backend). */
  qrSvg: string | null;
  connection: JamConnection;
  /** Reason for a reconnection or a failure, already translated. */
  reason: string | null;
  /** Name of the guest of each track added by the Jam, by track identifier. */
  guests: Record<string, string>;
  /** Host rules (#222, #288), kept from one Jam to the next. */
  rules: JamRules;
  /** Guests can add (suspension specific to the Jam, #288). */
  addsOpen: boolean;
  /** Guests who added during the Jam, banned ones included (#288). */
  members: JamGuest[];
}

/** Host rules for the Jam (#288). */
export interface JamRules {
  /** Guests can pause and resume (#222). */
  guestControls: boolean;
  /** Pending tracks at most per guest; `null` = no limit. */
  maxPending: number | null;
  /** Guests can add a whole album. */
  albums: boolean;
  /** Guests can set the volume (#449). */
  guestVolume: boolean;
  /** Highest volume a guest can request (1 to 100). */
  guestVolumeMax: number;
  /** Guests can vote to skip the current track (#433). */
  skipVote: boolean;
  /** Votes that skip the track (2 to 10). */
  skipVotes: number;
}

/** Jam guest as seen by the host (#288). */
export interface JamGuest {
  id: number;
  name: string | null;
  /** Tracks added during the Jam. */
  added: number;
  banned: boolean;
}

/** `jam-guests` event: names of the tracks and guests. */
export interface JamGuestsUpdate {
  guests: Record<string, string>;
  members: JamGuest[];
}

export const commands = {
  authStatus: () => invoke<SessionInfo | null>("auth_status"),
  login: () => invoke<SessionInfo>("login"),
  /** True if sign-in goes through the system browser (Linux, #17). */
  loginInBrowser: () => invoke<boolean>("login_in_browser"),
  /** Opens the sign-in page in the system browser and returns its address. */
  browserLoginStart: () => invoke<string>("browser_login_start"),
  /** Address of the page TIDAL redirected to after sign-in. */
  browserLoginFinish: (redirect: string) =>
    invoke<SessionInfo>("browser_login_finish", { redirect }),
  logout: () => invoke<void>("logout"),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  /** Latest searches kept, most recent first (#211). */
  searchHistory: () => invoke<string[]>("search_history"),
  /** Keeps a query in the history (#248: `search` no longer does it). */
  noteSearchHistory: (query: string) => invoke<string[]>("note_search_history", { query }),
  removeSearchHistory: (query: string) => invoke<string[]>("remove_search_history", { query }),
  clearSearchHistory: () => invoke<string[]>("clear_search_history"),
  /** Local listening history (#202): recent and most listened albums. */
  playHistory: () => invoke<PlayHistory>("play_history"),
  clearPlayHistory: () => invoke<void>("clear_play_history"),
  playStats: (period: StatsPeriod) => invoke<PlayStats>("play_stats", { period }),
  recapPeriods: () => invoke<RecapPeriod[]>("recap_periods"),
  recap: (year: number, month: number | null) => invoke<Recap>("recap", { year, month }),
  smartList: (list: SmartList) => invoke<Track[]>("smart_list", { list }),
  /** Local Jam (#25), desktop only: `null` if no Jam is running. */
  jamStatus: () => invoke<JamStatus | null>("jam_status"),
  jamOnlineAvailable: () => invoke<boolean>("jam_online_available"),
  /** Relay (#452): the chosen address is checked (`/healthz`) before being saved. */
  listenbrainzStatus: () => invoke<ListenBrainzStatus>("listenbrainz_status"),
  listenbrainzConnect: (token: string) =>
    invoke<ListenBrainzStatus>("listenbrainz_connect", { token }),
  listenbrainzDisconnect: () => invoke<ListenBrainzStatus>("listenbrainz_disconnect"),
  relaySettings: () => invoke<RelayView>("relay_settings"),
  setRelaySettings: (change: RelayChange) => invoke<RelayView>("set_relay_settings", { change }),
  /** Compiled updates folder (#452); `null`: no updates. */
  updateServer: () => invoke<string | null>("update_server"),
  /** Online, the address comes afterwards through `jam-status`. */
  startJam: (mode: JamMode) => invoke<JamStatus>("start_jam", { mode }),
  stopJam: () => invoke<void>("stop_jam"),
  /** Host rules, saved and applied (#222, #288); `null` without a Jam. */
  setJamRules: (rules: JamRules) => invoke<JamStatus | null>("set_jam_rules", { rules }),
  /** Suspends (`false`) or reopens the guests' additions (#288). */
  setJamAddsOpen: (open: boolean) => invoke<JamStatus | null>("set_jam_adds_open", { open }),
  /** Bans a guest (their pending tracks are removed) or readmits them (#288). */
  setJamGuestBanned: (id: number, banned: boolean) =>
    invoke<JamStatus | null>("set_jam_guest_banned", { id, banned }),
  /** Remote control (#336): `null` if it is off. */
  remoteStatus: () => invoke<RemoteStatus | null>("remote_status"),
  /** Turns the remote control on (choice and code kept). */
  startRemote: () => invoke<RemoteStatus | null>("start_remote"),
  stopRemote: () => invoke<RemoteStatus | null>("stop_remote"),
  /** #414: "Accept transfers"; `null` if the server stops. */
  setHandoffAccepted: (enabled: boolean) =>
    invoke<RemoteStatus | null>("set_handoff_accepted", { enabled }),
  /** #414, phone: linked computer (never the code). */
  linkedComputer: () => invoke<LinkedComputer | null>("linked_computer"),
  /** Address of the PC's remote control with its code after `#`, checked before being kept. */
  linkComputer: (link: string) => invoke<LinkedComputer>("link_computer", { link }),
  unlinkComputer: () => invoke<void>("unlink_computer"),
  /** "Resume here": playback of the PC moves to the phone, the PC pauses. */
  handoffPull: () => invoke<void>("handoff_pull"),
  /** "Continue on the computer": the phone pauses, the PC resumes. */
  handoffPush: () => invoke<void>("handoff_push"),
  /** #430: listening history exchanged with the linked computer; date of the last
   *  successful sync, `null` without a linked computer. Without `force`, nothing if recent. */
  syncHistory: (force: boolean) => invoke<number | null>("sync_history", { force }),
  /** New code: paired phones are disconnected. */
  renewRemoteCode: () => invoke<RemoteStatus | null>("renew_remote_code"),
  /** #62: "Link a phone", new relay key and 10 minutes for a phone to learn the route. */
  openHandoffPairing: () => invoke<RemoteStatus | null>("open_handoff_pairing"),
  album: (id: number) => invoke<AlbumPage>("album", { id }),
  artist: (id: number) => invoke<ArtistPage>("artist", { id }),
  /** Other versions and other albums by the artist (#350), displayed album excluded. */
  albumRelated: (id: number) => invoke<AlbumRelated>("album_related", { id }),
  /** Biography, similar artists, "Appears on" (#205): each part can be empty. */
  artistExtra: (id: number) => invoke<ArtistExtra>("artist_extra", { id }),
  /** `null` if the playlist has not changed since `revision` (#152). */
  playlist: (uuid: string, revision: string | null = null) =>
    invoke<PlaylistPage | null>("playlist", { uuid, revision }),
  /** Copy kept by the backend, without network: displayed while waiting for `playlist`. */
  cachedPlaylist: (uuid: string) => invoke<PlaylistPage | null>("cached_playlist", { uuid }),
  /** Copy of the tab saved at the last launch (#181), without network; no mixes. */
  cachedLibrary: (tab: "tracks" | "albums" | "artists" | "playlists") =>
    invoke<LibraryList<unknown> | null>("cached_library", { tab }),
  mix: (id: string) => invoke<MixPage>("mix", { id }),
  radio: (seed: RadioSeed, id: number) => invoke<RadioPage>("radio", { seed, id }),
  libraryMixes: () => invoke<MixSummary[]>("library_mixes"),
  libraryTracks: () => invoke<LibraryList<TrackEntry>>("library_tracks"),
  libraryAlbums: () => invoke<LibraryList<AlbumSummary>>("library_albums"),
  libraryArtists: () => invoke<LibraryList<ArtistSummary>>("library_artists"),
  libraryPlaylists: () => invoke<LibraryList<PlaylistSummary>>("library_playlists"),
  home: () => invoke<HomeFeed>("home"),
  /** The whole home, following pages included: replaces the first page. */
  homeRest: () => invoke<HomeSection[]>("home_rest"),
  /** Completed feed of releases by followed artists (#415); also emits `feed-unseen`. */
  feed: () => invoke<FeedItem[]>("feed"),
  /** Marks the whole last feed read as seen (page opening). */
  markFeedSeen: () => invoke<void>("mark_feed_seen"),
  /** `null` if TIDAL has no lyrics for this track. */
  lyrics: (trackId: TrackId) => invoke<Lyrics | null>("lyrics", { trackId }),
  /** Actual format, ReplayGain, BPM…: a read request, to only ask for on demand. */
  trackInfo: (trackId: TrackId) => invoke<TrackInfo>("track_info", { trackId }),
  /** TIDAL link contained in a pasted text, or `null` (a track resolves to its album). */
  openLink: (text: string) => invoke<LinkTarget | null>("open_link", { text }),
  /** Reads a playlist from another service and looks for its tracks (#328); progress through
   *  `onImportProgress`. Nothing is saved. */
  importPlaylist: (link: string) => invoke<ImportPreview>("import_playlist", { link }),
  /** Stops the import in progress (page left). */
  cancelImport: () => invoke<void>("cancel_import"),
  /** Credits of the whole album, on demand (one request per page of 100 tracks). */
  albumCredits: (albumId: number) => invoke<AlbumCreditGroup[]>("album_credits", { albumId }),
  /** Batch of tracks of a contributor (#429): `offset` = previous `nextOffset`, 0 at the start. */
  contributor: (artistId: number, offset: number) =>
    invoke<ContributorPage>("contributor", { artistId, offset }),
  /** Hi-res album with the same title, for an album that is not (#59). */
  hiresTwinAlbum: (albumId: number) => invoke<AlbumSummary | null>("hires_twin_album", { albumId }),
  /** Identifiers of all favorites (updated afterwards by the `likes` event). */
  likes: () => invoke<LikedIds>("likes"),
  /** Adds to or removes from favorites; the `likes` event follows the confirmation. */
  setLiked: (target: LikeTarget, liked: boolean) => invoke<void>("set_liked", { target, liked }),
  /** Several tracks added to or removed from favorites at once (selection, #332). */
  setTracksLiked: (trackIds: TrackId[], liked: boolean) =>
    invoke<void>("set_tracks_liked", { trackIds, liked }),
  /** Blocked tracks and artists (#334), read from the service. */
  blocked: () => invoke<Blocked>("blocked"),
  /** Blocks or unblocks a track or an artist. */
  setBlocked: (target: BlockTarget, blocked: boolean) =>
    invoke<void>("set_blocked", { target, blocked }),
  /** Identifiers of "Save for later" (#196), updated afterwards by the `later` event. */
  laterIds: () => invoke<LikedIds>("later_ids"),
  /** The whole "Save for later" list, from newest to oldest. */
  saveForLater: () => invoke<LibraryList<LaterItem>>("save_for_later"),
  /** Adds to or removes from "Save for later" (no mix); the `later` event follows. */
  setSavedForLater: (target: LikeTarget, saved: boolean) =>
    invoke<void>("set_saved_for_later", { target, saved }),
  /** Playlists created by the user, the only editable ones. */
  ownPlaylists: () => invoke<PlaylistSummary[]>("own_playlists"),
  /** Adds at the end, without duplicates; returns the number of tracks actually added. */
  addToPlaylist: (uuid: string, trackIds: TrackId[]) =>
    invoke<number>("add_to_playlist", { uuid, trackIds }),
  /** Creates a playlist with these tracks; returns its uuid. */
  createPlaylist: (title: string, trackIds: TrackId[]) =>
    invoke<string>("create_playlist", { title, trackIds }),
  /** `index`: position in the playlist (0 = first); refused if another track is there. */
  removeFromPlaylist: (uuid: string, index: number, trackId: TrackId) =>
    invoke<void>("remove_from_playlist", { uuid, index, trackId }),
  /** Removes several positions (#332); everything is refused if one has changed. */
  removeManyFromPlaylist: (uuid: string, items: PlaylistItem[]) =>
    invoke<void>("remove_many_from_playlist", { uuid, items }),
  /** Undoes a batch removal; returns the number of tracks put back. */
  restoreManyToPlaylist: (uuid: string, items: PlaylistItem[]) =>
    invoke<number>("restore_many_to_playlist", { uuid, items }),
  /** Puts a removed track back at its position (undo, #349); `false` if nothing was added. */
  restoreToPlaylist: (uuid: string, index: number, trackId: TrackId) =>
    invoke<boolean>("restore_to_playlist", { uuid, index, trackId }),
  /** Moves the track from `from` to `to` (final position, 0 = first, #201). */
  moveInPlaylist: (uuid: string, from: number, to: number, trackId: TrackId) =>
    invoke<void>("move_in_playlist", { uuid, from, to, trackId }),
  /** Renames and changes the description (#201). */
  editPlaylist: (uuid: string, title: string, description: string) =>
    invoke<void>("edit_playlist", { uuid, title, description }),
  deletePlaylist: (uuid: string) => invoke<void>("delete_playlist", { uuid }),
  /** Playlist folders (#331), sorted by name; empty if the user has none. */
  playlistFolders: () => invoke<PlaylistFolder[]>("playlist_folders"),
  /** Creates a folder at the root, with these playlists inside. */
  createFolder: (name: string, uuids: string[]) =>
    invoke<PlaylistFolder>("create_folder", { name, uuids }),
  renameFolder: (id: string, name: string) => invoke<void>("rename_folder", { id, name }),
  /** Puts playlists in a folder, or moves them back to the root (`folderId` null). */
  moveToFolder: (uuids: string[], folderId: string | null) =>
    invoke<void>("move_to_folder", { uuids, folderId }),
  /** Deletes a folder; its playlists are kept, at the root. */
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  /** Duplicates and unavailable tracks of one of your playlists (#382). */
  playlistCleanup: (uuid: string) => invoke<CleanupReport>("playlist_cleanup", { uuid }),
  /** Applies the chosen changes (positions of the report, in any order). */
  cleanPlaylist: (uuid: string, ops: ListOp[]) =>
    invoke<ListOpsResult>("clean_playlist", { uuid, ops }),
  /** Undoes: `ops` = the `undo` of the cleanup. */
  undoPlaylistCleanup: (uuid: string, ops: ListOp[]) =>
    invoke<ListOpsResult>("undo_playlist_cleanup", { uuid, ops }),
  /** Duplicates and unavailable tracks among the liked tracks (#382). */
  likedCleanup: () => invoke<CleanupReport>("liked_cleanup"),
  /** Applies changes to the liked tracks, or undoes them; the `likes` event follows. */
  cleanLiked: (ops: ListOp[]) => invoke<ListOpsResult>("clean_liked", { ops }),
  /** Blind test (#434): game of `rounds` rounds drawn from `trackIds`. */
  /** With `jam`, the guests of the current Jam play too (#441). */
  blindTestStart: (
    trackIds: TrackId[],
    mode: BlindMode,
    rounds: number,
    title: string,
    jam = false,
    hostPlays = true,
  ) => invoke<void>("blind_test_start", { trackIds, mode, rounds, title, jam, hostPlays }),
  /** Reveals the round without waiting for the missing answers (with several players). */
  blindTestReveal: () => invoke<void>("blind_test_reveal"),
  blindTestAnswer: (answer: BlindAnswer) => invoke<void>("blind_test_answer", { answer }),
  blindTestReplay: () => invoke<void>("blind_test_replay"),
  blindTestNext: () => invoke<void>("blind_test_next"),
  blindTestQuit: () => invoke<void>("blind_test_quit"),
  blindTestState: () => invoke<BlindTestView | null>("blind_test_state"),
  /** `shuffle`: `true` turns shuffle on and starts on a random track, `false` turns it off
   *  ("Play" buttons, #526), `null` keeps the current mode (a track picked in a list). */
  playTracks: (
    trackIds: TrackId[],
    startIndex: number,
    shuffle: boolean | null = null,
    source: PlaySource | null = null,
  ) => invoke<void>("play_tracks", { trackIds, startIndex, shuffle, source }),
  /** Android: true once after the `seaglass://drive` link or the app shortcut (#193). */
  takeDriveRequest: () => invoke<boolean>("take_drive_request"),
  /** Android: pin requested by an icon shortcut or the `seaglass://pin/…` link
   *  (#335), only once. */
  takePinRequest: () => invoke<Pin | null>("take_pin_request"),
  /** Android: screen kept on (driving mode, #193); nothing on desktop. */
  keepScreenOn: (on: boolean) => invoke<void>("keep_screen_on", { on }),
  playNext: (trackId: TrackId) => invoke<void>("play_next", { trackId }),
  addToQueue: (trackId: TrackId) => invoke<void>("add_to_queue", { trackId }),
  /** Several tracks in order: at the head of the queue with `next` ("Play next"), else at the end. */
  enqueueTracks: (trackIds: TrackId[], next: boolean) =>
    invoke<void>("enqueue_tracks", { trackIds, next }),
  removeFromQueue: (index: number) => invoke<void>("remove_from_queue", { index }),
  /** Moves the entry `from` of the manual queue so that it ends at index `to`. */
  moveInQueue: (from: number, to: number) => invoke<void>("move_in_queue", { from, to }),
  clearQueue: () => invoke<void>("clear_queue"),
  /** Undoes the last clearing or removal of a track from the queue (#349); `false` if the
   *  queue has changed since. */
  restoreQueue: () => invoke<boolean>("restore_queue"),
  /** Plays the track `index` of `QueueView.manual` or `.upcoming` (#167); no effect if it is no
   *  longer `trackId` (queue changed since the display). */
  playFromQueue: (part: QueuePart, index: number, trackId: TrackId) =>
    invoke<void>("play_from_queue", { part, index, trackId }),
  /** Answer to the "Hi-res edition available" banner, valid for the whole album (#59). */
  answerHiresOffer: (accept: boolean) => invoke<void>("answer_hires_offer", { accept }),
  /** Pause after a duration or at the end of the current track; `null` cancels (#85). */
  setSleepTimer: (timer: SleepRequest | null) => invoke<void>("set_sleep_timer", { timer }),
  next: () => invoke<void>("next"),
  previous: () => invoke<void>("previous"),
  togglePause: () => invoke<void>("toggle_pause"),
  seek: (seconds: number) => invoke<void>("seek", { seconds }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: Repeat) => invoke<void>("set_repeat", { mode }),
  /** 0 to 100; restores the sound if it was muted (except at 0). */
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setMuted: (muted: boolean) => invoke<void>("set_muted", { muted }),
  playerState: () => invoke<PlayerState>("player_state"),
  queueState: () => invoke<QueueView>("queue_state"),
  queueTrackIds: () => invoke<number[]>("queue_track_ids"),
  uiSettings: () => invoke<UiSettings>("ui_settings"),
  setUiSettings: (settings: UiSettings) => invoke<void>("set_ui_settings", { settings }),
  globalShortcutsStatus: () => invoke<GlobalShortcutsStatus>("global_shortcuts_status"),
  /** Effective language (setting, or system language). */
  language: () => invoke<Lang>("language"),
  /** Saves the setting; returns the effective language. */
  setLanguage: (language: LanguagePref) => invoke<Lang>("set_language", { language }),
  /** Sections displayed, in screen order; returns the saved layout. */
  setHomeLayout: (sections: { key: string; hidden: boolean }[]) =>
    invoke<HomeLayout>("set_home_layout", { sections }),
  /** Shows or hides the "Featured" banner (#185); returns the saved layout. */
  setHomeFeatured: (shown: boolean) => invoke<HomeLayout>("set_home_featured", { shown }),
  resetHomeLayout: () => invoke<HomeLayout>("reset_home_layout"),
  /** Opens the website, the repository or a new issue in the browser (#188, #517); fixed URLs on the Rust side. */
  openProjectPage: (page: ProjectPage) => invoke<void>("open_project_page", { page }),
  /** #192: true only once if the previous launch crashed. */
  takeCrashNotice: () => invoke<boolean>("take_crash_notice"),
  /** Report of the last crash, to copy into an issue. */
  lastCrashReport: () => invoke<string | null>("last_crash_report"),
  /** End of the log of this launch. */
  currentLog: () => invoke<string | null>("current_log"),
  /** #352: settings of this device as JSON, to paste elsewhere. */
  exportSettings: () => invoke<string>("export_settings"),
  /** Restores a pasted backup; keys `ui.theme`, `playback.autoRadio`… */
  importSettings: (text: string) => invoke<ImportReport>("import_settings", { text }),
  changelog: () => invoke<Release[]>("changelog"),
  /** Desktop: log folder in the explorer. */
  openLogDir: () => invoke<void>("open_log_dir"),
  /** Desktop (#330): exports the library as JSON and CSV in Documents\Seaglass. */
  exportLibrary: () => invoke<LibraryExportReport>("export_library"),
  cancelLibraryExport: () => invoke<void>("cancel_library_export"),
  /** Replaces the pins; returns the saved list (validated, without duplicates). */
  setPins: (pins: Pin[]) => invoke<Pin[]>("set_pins", { pins }),
  /** Remembers the sort of a list (`null`: forgets it); returns the saved sorts. */
  setListSort: (list: string, sort: TrackSort | null) =>
    invoke<ListSort[]>("set_list_sort", { list, sort }),
  playbackSettings: () => invoke<PlaybackSettings>("playback_settings"),
  /** Saves and applies live (without reloading the current track). */
  setPlaybackSettings: (settings: PlaybackSettings) =>
    invoke<void>("set_playback_settings", { settings }),
  /** Android (#492): battery cap of the moment; `null` if the battery changes nothing. */
  qualityCap: () => invoke<QualityCap | null>("quality_cap"),
  /** #428: parses an AutoEQ "ParametricEQ.txt"; profile with identifier 0, no name. */
  parseAutoeq: (text: string) => invoke<EqProfile>("parse_autoeq", { text }),
  /** Output devices known to mpv; the rest arrives through `audio-devices`. */
  audioDevices: () => invoke<AudioDevices>("audio_devices"),
  /** Platform of the app: chooses the layout (touch on Android). */
  platform: () => invoke<Platform>("platform"),
  /** Actual launch-at-startup state, read from the system. */
  autostartEnabled: () => invoke<boolean>("autostart_enabled"),
  /** True if Discord presence can be offered (desktop, Discord app compiled, #215). */
  discordAvailable: () => invoke<boolean>("discord_available"),
  /** True if exclusive mode can be offered (Windows only, #245, #17). */
  exclusiveAvailable: () => invoke<boolean>("exclusive_available"),
  /** Returns the state read again after the change. */
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  /** Desktop only: full screen of the main window (#131). */
  /** Android: true once after a tap on the notification (#163). */
  takeNowPlayingRequest: () => invoke<boolean>("take_now_playing_request"),
  /** Android: link received through the "Share" menu (#203), only once. */
  takeSharedLink: () => invoke<LinkTarget | null>("take_shared_link"),
  /** Android 12+: `https://tidal.com/…` links given to the app; `null` = nothing to set (#203). */
  linkHandling: () => invoke<{ enabled: boolean } | null>("link_handling"),
  /** Android: opens the app's "Open by default" setting. */
  openLinkSettings: () => invoke<void>("open_link_settings"),
  fullscreen: () => invoke<boolean>("fullscreen"),
  /** Returns the state read again; the rest also arrives through `fullscreen`. */
  setFullscreen: (fullscreen: boolean) => invoke<boolean>("set_fullscreen", { fullscreen }),
  /** Desktop mini player (#384): opening it hides the main window, closing it shows it again. */
  openMiniPlayer: () => invoke<void>("open_mini_player"),
  closeMiniPlayer: () => invoke<void>("close_mini_player"),
  miniPlayerOnTop: () => invoke<boolean>("mini_player_on_top"),
  /** Returns the kept state. */
  setMiniPlayerOnTop: (onTop: boolean) => invoke<boolean>("set_mini_player_on_top", { onTop }),
  updateStatus: () => invoke<UpdateInfo | null>("update_status"),
  /** False on Linux outside an AppImage: updates to do by hand (#314). */
  updatesAvailable: () => invoke<boolean>("updates_available"),
  /** The installer closes the app then relaunches it (Linux: AppImage replaced, then relaunch). */
  installUpdate: () => invoke<void>("install_update"),
  /** Checks right away: the update ready (re-emitted as `update-ready`), being
   *  downloaded (`downloading`, `update-ready` will follow), or `null` if up to date. */
  checkForUpdates: () => invoke<UpdateInfo | null>("check_for_updates"),
};

export const events = {
  /** #430: listens received from the other device (count): reload the displayed history. */
  onHistorySynced: (cb: (added: number) => void): Promise<UnlistenFn> =>
    listen<number>("history-synced", (e) => cb(e.payload)),
  /** Jam (#25): connection to the relay (connected, reconnection, new address). */
  onJamStatus: (cb: (status: JamStatus) => void): Promise<UnlistenFn> =>
    listen<JamStatus>("jam-status", (e) => cb(e.payload)),
  /** Jam (#25): a guest added or removed tracks, or was banned (#288). */
  onJamGuests: (cb: (update: JamGuestsUpdate) => void): Promise<UnlistenFn> =>
    listen<JamGuestsUpdate>("jam-guests", (e) => cb(e.payload)),
  onPlayerState: (cb: (s: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("player-state", (e) => cb(e.payload)),
  onBlindTest: (cb: (view: BlindTestView | null) => void): Promise<UnlistenFn> =>
    listen<BlindTestView | null>("blind-test", (e) => cb(e.payload)),
  onQueueState: (cb: (q: QueueView) => void): Promise<UnlistenFn> =>
    listen<QueueView>("queue-state", (e) => cb(e.payload)),
  onUpdateReady: (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>("update-ready", (e) => cb(e.payload)),
  /** Desktop: the installer download failed (error message, #187). */
  onUpdateFailed: (cb: (message: string) => void): Promise<UnlistenFn> =>
    listen<string>("update-failed", (e) => cb(e.payload)),
  /** #330: progress of the library export. */
  /** Playlist import (#328): tracks searched on TIDAL. */
  onImportProgress: (cb: (p: ImportProgress) => void): Promise<UnlistenFn> =>
    listen<ImportProgress>("import-progress", (e) => cb(e.payload)),
  onLibraryExport: (cb: (p: LibraryExportProgress) => void): Promise<UnlistenFn> =>
    listen<LibraryExportProgress>("library-export", (e) => cb(e.payload)),
  onLikes: (cb: (ids: LikedIds) => void): Promise<UnlistenFn> =>
    listen<LikedIds>("likes", (e) => cb(e.payload)),
  onLater: (cb: (ids: LikedIds) => void): Promise<UnlistenFn> =>
    listen<LikedIds>("later", (e) => cb(e.payload)),
  onAudioDevices: (cb: (d: AudioDevices) => void): Promise<UnlistenFn> =>
    listen<AudioDevices>("audio-devices", (e) => cb(e.payload)),
  /** Playback settings changed by the backend (profile of an output device, #416):
   *  to read again. */
  onPlaybackSettings: (cb: () => void): Promise<UnlistenFn> =>
    listen<null>("playback-settings", () => cb()),
  /** Android: tap on the notification (#163); the request is taken with `takeNowPlayingRequest`. */
  onOpenNowPlaying: (cb: () => void): Promise<UnlistenFn> =>
    listen<null>("open-now-playing", () => cb()),
  /** Android: driving mode requested (#193); the request is taken with `takeDriveRequest`. */
  onOpenDrive: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-drive", () => cb()),
  /** Android: pin to read (#335); it is taken with `takePinRequest`. */
  onOpenPin: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-pin", () => cb()),
  /** Shared link ready (Android, #203) or notification tapped (#435); it is taken with
   * `takeSharedLink`. */
  onSharedLink: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-link", () => cb()),
  /** Global shortcut run by the UI (#413): "Like" only. */
  onGlobalShortcut: (cb: (action: GlobalAction) => void): Promise<UnlistenFn> =>
    listen<GlobalAction>("global-shortcut", (e) => cb(e.payload)),
  onFullscreen: (cb: (fullscreen: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("fullscreen", (e) => cb(e.payload)),
  /** TIDAL refused the refresh midway: sign-in needed again (#259). */
  onSessionLost: (cb: () => void): Promise<UnlistenFn> => listen<null>("session-lost", () => cb()),
  /** Number of unseen news from the feed (#415). */
  onFeedUnseen: (cb: (count: number) => void): Promise<UnlistenFn> =>
    listen<number>("feed-unseen", (e) => cb(e.payload)),
  onToast: (cb: (t: Toast) => void): Promise<UnlistenFn> =>
    listen<Toast>("toast", (e) => cb(e.payload)),
};
