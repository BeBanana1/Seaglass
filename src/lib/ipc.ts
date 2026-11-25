// Typed wrappers for the Tauri commands and events.
// Types generated from Rust by ts-rs live in `bindings/` (#44, `cargo test` writes them) and are
// re-exported here; the others are still copied by hand from Rust (tidal/models.rs,
// player/state.rs…) until their group moves to `bindings/`.

import { invoke, listen, type UnlistenFn } from "./transport";
import type { AlbumRef } from "./bindings/AlbumRef";
import type { ArtistRef } from "./bindings/ArtistRef";
import type { AudioParams } from "./bindings/AudioParams";
import type { PlayerState } from "./bindings/PlayerState";
import type { Repeat } from "./bindings/Repeat";
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
export type { PlayerState } from "./bindings/PlayerState";
export type { Repeat } from "./bindings/Repeat";
export type { SleepState } from "./bindings/SleepState";
export type { StreamFormat } from "./bindings/StreamFormat";
export type { Track } from "./bindings/Track";
export type { TrackId } from "./bindings/TrackId";
export type { Volume } from "./bindings/Volume";

/** Résultat d'un import de réglages (#352). */
/** #330 : export de la bibliothèque terminé (dossier déjà ouvert). */
export interface LibraryExportReport {
  dir: string;
  tracks: number;
  albums: number;
  artists: number;
  playlists: number;
  /** Titres des playlists illisibles, laissées de côté. */
  failed: string[];
  /** Une liste au moins est coupée au plafond de chargement. */
  truncated: boolean;
}

/** #330 : playlists lues sur le total. */
export interface LibraryExportProgress {
  done: number;
  total: number;
}

export interface ImportReport {
  applied: string[];
  ignored: string[];
}

/** Titre d'une liste (playlist ou favoris) avec sa date d'ajout à cette liste. */
export interface TrackEntry extends Track {
  /** Millisecondes depuis l'epoch Unix (UTC) ; `null` si l'API ne la donne pas. */
  addedAt: number | null;
}

/** Une catégorie de la bibliothèque, du plus récent au plus ancien (#91). */
export interface LibraryList<T> {
  items: T[];
  /** Coupée au plafond de chargement : seuls les plus récents sont là. */
  truncated: boolean;
  /** Nombre annoncé par TIDAL (indisponibles compris), s'il est connu. */
  total: number | null;
}

/** `commands::project::ProjectPage`: project page opened in the browser (#188, #517). */
export type ProjectPage = "website" | "repo" | "newIssue" | "clientId";

/** Infos détaillées d'un titre (#3), demandées à l'ouverture du panneau. */
export interface TrackInfo {
  track: Track;
  /** `LOSSLESS`, `HIRES_LOSSLESS`, `DOLBY_ATMOS`… */
  tags: string[];
  /** Format servi en qualité maximale ; `null` si refusé (voir `streamError`). */
  stream: StreamFormat | null;
  streamError: string | null;
  /** ReplayGain en dB et crêtes (pleine échelle = 1), jamais appliqués par Seaglass. */
  trackGain: number | null;
  trackPeak: number | null;
  albumGain: number | null;
  albumPeak: number | null;
  bpm: number | null;
  /** Tonalité brute de l'API (`B`, `FSharp`, `Bb`…) et mode (`MAJOR`, `MINOR`). */
  key: string | null;
  keyScale: string | null;
  copyright: string | null;
  trackNumber: number | null;
  volumeNumber: number | null;
  /** Autre édition hi-res du même enregistrement, si le titre n'est pas hi-res. */
  hiresEdition: HiresEdition | null;
  /** Crédits groupés par rôle, rangés par le backend ; vide si TIDAL n'en a pas (#84). */
  credits: CreditGroup[];
}

export interface CreditGroup {
  /** Texte libre en anglais (`Composer`, `Bass guitar`…), traduit par `creditRole`. */
  role: string;
  contributors: Contributor[];
}

export interface Contributor {
  name: string;
  /** Artiste TIDAL (page artiste), s'il y en a un. */
  artistId: number | null;
}

/** Page à ouvrir pour un lien TIDAL collé (#203). */
export type LinkTarget =
  | { kind: "album"; id: number; track: TrackId | null }
  | { kind: "artist"; id: number }
  | { kind: "playlist"; uuid: string }
  | { kind: "mix"; id: string }
  /** Playlist d'un autre service à importer (#328) : adresse propre, relue par `importPlaylist`. */
  | { kind: "import"; link: string }
  /** Fil d'actualité : notification groupée des nouvelles sorties (#435). */
  | { kind: "feed" };

/** Import d'une playlist d'un autre service (#328) : un titre d'origine et sa correspondance. */
export interface ImportItem {
  title: string;
  artist: string | null;
  /** `null` : rien de ressemblant sur TIDAL. */
  track: Track | null;
  /** Titre, artiste et durée concordent (ou même ISRC) ; sinon à vérifier. */
  sure: boolean;
}

export type ImportService = "deezer" | "spotify" | "apple";

export interface ImportPreview {
  name: string;
  service: ImportService;
  /** Titres de la playlist chez le service. */
  total: number;
  /** Titres lus : moins que `total` si le service n'en montre qu'une partie (Spotify : 100). */
  read: number;
  items: ImportItem[];
}

export interface ImportProgress {
  done: number;
  total: number;
}

/** Crédits d'un album entier (#206). */
export interface AlbumCreditGroup {
  role: string;
  contributors: AlbumContributor[];
}

/** Titres où une personne est créditée (#429), par lots. */
export interface ContributorPage {
  artist: ArtistSummary;
  /** Nombre de titres annoncé par le service. */
  total: number;
  tracks: Contribution[];
  /** À repasser à `contributor` pour le lot suivant ; `null` à la fin. */
  nextOffset: number | null;
}

export interface Contribution {
  track: Track;
  /** Rôles en anglais de l'API (`Producer`, `Composer`…), traduits par `creditRole`. */
  roles: string[];
}

export interface AlbumContributor {
  name: string;
  artistId: number | null;
  /** Titres où il apparaît, dans l'ordre de l'album ; vide = tous. */
  tracks: TrackId[];
}

export interface HiresEdition {
  track: Track;
  stream: StreamFormat | null;
  /** Seaglass la lit à la place du titre (réglage actif et flux meilleur). */
  played: boolean;
}

/** Périphérique de sortie WASAPI (#79) : identifiant mpv (`wasapi/{guid}`) et nom Windows. */
export interface AudioDevice {
  id: string;
  name: string;
}

/** Périphériques connus de mpv (commande `audio_devices`, événement `audio-devices`). */
export interface AudioDevices {
  devices: AudioDevice[];
  /** Identifiant du périphérique par défaut de Windows, s'il est connu. */
  systemDefault: string | null;
}

/** Blind test (#434) : saisie libre ou quatre propositions. */
export type BlindMode = "typing" | "choices";

export type BlindAnswer =
  { kind: "text"; text: string } | { kind: "choice"; index: number } | { kind: "pass" };

export interface BlindChoice {
  title: string;
  artist: string;
}

/** Joueur d'une partie (#441) : l'hôte (`id` 0) ou un invité de la Jam. */
export interface BlindPlayer {
  id: number;
  /** Prénom donné dans la Jam ; `null` : « Invité <id> ». */
  name: string | null;
  host: boolean;
  score: number;
  streak: number;
  bestStreak: number;
  /** A répondu à la manche en cours. */
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
  /** Premier à trouver le titre (bonus compris dans `points`). */
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
  /** Tous les joueurs, du plus de points au moins (#441). */
  players: BlindPlayerResult[];
}

/** Partie tenue par le backend (événement `blind-test`) ; le titre de la manche n'y est
 *  qu'une fois révélé, dans `results`. */
export interface BlindTestView {
  title: string;
  mode: BlindMode;
  phase: "guess" | "revealed" | "finished";
  /** Manche en cours, à partir de 1. */
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
  /** Classement ; l'hôte seul en solo. */
  players: BlindPlayer[];
  /** Révélation automatique prévue (ms depuis l'epoch), à plusieurs. */
  revealAt: number | null;
  /** L'extrait joue vraiment (#445) : avant, le moteur charge encore le titre. */
  clipPlaying: boolean;
}

export type SleepRequest =
  { kind: "minutes"; minutes: number } | { kind: "endOfTrack" } | { kind: "endOfContext" };

/** Partie de `QueueView` d'où lire un titre (#167). */
export type QueuePart = "manual" | "upcoming";

export interface QueueView {
  manual: Track[];
  upcoming: Track[];
  /** Titre de départ quand la suite est une radio qui prolonge la file (#78). */
  radio: Track | null;
  /** D'où vient le titre en cours (« Joué depuis », #124), s'il vient du contexte. */
  source: PlaySource | null;
}

/** Origine d'un contexte de lecture (#124), passée à `playTracks`. */
export type PlaySource =
  | { kind: "album"; id: number; title: string }
  | { kind: "playlist"; uuid: string; title: string }
  | { kind: "mix"; id: string; title: string }
  | { kind: "radio"; seed: RadioSeed; id: number; title: string }
  | { kind: "artist"; id: number; name: string }
  | { kind: "contributor"; id: number; name: string }
  | { kind: "likedTracks" }
  | { kind: "later" }
  | { kind: "search"; query: string }
  | { kind: "smart"; list: SmartList }
  | { kind: "import"; link: string; title: string };

export interface AlbumSummary {
  id: number;
  title: string;
  artists: ArtistRef[];
  coverUrl: string | null;
  /** 640×640 (#89). */
  coverLargeUrl?: string | null;
  color: string | null;
  /** `AAAA-MM-JJ`. */
  releaseDate: string | null;
  numberOfTracks: number | null;
  /** `ALBUM`, `EP` ou `SINGLE`. */
  albumType: string | null;
  explicit: boolean;
  /** Généré par IA selon le service (#369). */
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

/** Dossier de playlists de la bibliothèque (#331), un seul niveau. */
export interface PlaylistFolder {
  id: string;
  name: string;
  /** Uuids des playlists rangées (les vôtres et celles que vous suivez). */
  playlists: string[];
}

/** Mix généré par le service (My Mix, Daily Discovery, mix d'artiste…). */
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

/** Élément de « Plus tard » (#196) : un titre avec sa date d'ajout, ou une carte. */
export type LaterItem =
  | ({ kind: "track" } & TrackEntry)
  | ({ kind: "album" } & AlbumSummary)
  | ({ kind: "artist" } & ArtistSummary)
  | ({ kind: "playlist" } & PlaylistSummary);

/** Ce qu'on peut ajouter aux favoris (#4, #81). */
export type LikeTarget =
  | { kind: "track"; id: TrackId }
  | { kind: "album"; id: number }
  | { kind: "artist"; id: number }
  | { kind: "playlist"; uuid: string }
  | { kind: "mix"; id: string };

/** Ce qu'on peut bloquer (#334) : ni proposé par les mixes ni par les radios. */
export type BlockTarget = { kind: "track"; id: TrackId } | { kind: "artist"; id: number };

/** Titres et artistes bloqués, du plus récent au plus ancien. */
export interface Blocked {
  tracks: Track[];
  artists: ArtistSummary[];
}

/** Tous les favoris, en identifiants (événement `likes`). */
/** Position dans une playlist (0 = premier) et titre attendu à cette position (#332). */
export interface PlaylistItem {
  index: number;
  trackId: TrackId;
}

export interface LikedIds {
  tracks: TrackId[];
  albums: number[];
  artists: number[];
  /** Playlists suivies. */
  playlists: string[];
  /** Mixes enregistrés (#204). */
  mixes: string[];
}

export interface HomeRow {
  /** Identifiant stable de la section (`moduleId` de l'API), indépendant du titre. */
  key: string;
  title: string;
  items: MediaItem[];
}

/** Ligne de l'accueil dans l'ordre choisi ; les masquées ne sont montrées qu'en personnalisation. */
export interface HomeSection extends HomeRow {
  hidden: boolean;
}

/** Première page de l'accueil ; `more` : la suite vient de `homeRest` (#207). */
export interface HomeFeed {
  sections: HomeSection[];
  more: boolean;
}

/** Élément du fil d'actualité : sortie d'un artiste suivi (album) ou mix mensuel (mix). */
export interface FeedItem {
  /** Date de la sortie, en millisecondes UTC. */
  occurredAt: number | null;
  /** Déjà vue dans l'app (état local, #415 ; à la première lecture, repris de TIDAL). */
  seen: boolean;
  item: MediaItem;
}

/** Disposition de l'accueil enregistrée (clés de `HomeRow`). */
export interface HomeLayout {
  order: string[];
  hidden: string[];
  /** Bandeau « À la une » masqué (#185). */
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

/** Historique d'écoute local (#202). */
export interface PlayHistory {
  /** Albums écoutés, du plus récent au plus ancien. */
  recent: AlbumSummary[];
  /** Albums les plus écoutés ces 30 derniers jours (au moins deux écoutes). */
  top: AlbumSummary[];
}

/** Période des statistiques d'écoute (#326) : fenêtres glissantes de 7, 30, 365 jours. */
export type StatsPeriod = "week" | "month" | "year" | "all";

export interface Counted<T> {
  item: T;
  plays: number;
}

/** Statistiques d'écoute locales (#326), propres à cet appareil. */
export interface PlayStats {
  plays: number;
  /** Temps d'écoute approché (somme des durées des titres écoutés), en secondes. */
  seconds: number;
  /** Écoutes de durée inconnue (antérieures à la 0.5.33), absentes de `seconds`. */
  unknown: number;
  tracks: Counted<Track>[];
  albums: Counted<AlbumSummary>[];
  /** Photo demandée une fois à TIDAL, puis gardée avec l'historique. */
  artists: Counted<ArtistSummary>[];
}

/** Période de la rétrospective (#417) : mois civil, ou année entière sans `month`. */
export interface RecapPeriod {
  year: number;
  month: number | null;
}

/** Rétrospective d'un mois ou d'une année (#417), calculée sur l'historique de cet appareil. */
export interface Recap {
  plays: number;
  seconds: number;
  unknown: number;
  /** Jours avec au moins une écoute. */
  days: number;
  tracks: Counted<Track>[];
  albums: Counted<AlbumSummary>[];
  artists: Counted<ArtistSummary>[];
  /** Artistes sans aucune écoute avant la période. */
  newArtists: Counted<ArtistSummary>[];
  newArtistCount: number;
  newTrackCount: number;
  /** Écoutes par jour de la semaine, lundi en premier. */
  weekdays: number[];
  /** Écoutes par heure locale, de 0 à 23. */
  hours: number[];
  /** 0 = lundi. */
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

/**
 * Playlist intelligente locale (#383), calculée par le backend sur l'historique de cet
 * appareil (et les titres likés pour `neverPlayed`).
 */
export type SmartList = "topMonth" | "onRepeat" | "forgotten" | "neverPlayed";

export const SMART_LISTS: SmartList[] = ["topMonth", "onRepeat", "forgotten", "neverPlayed"];

/** Clé de la section d'accueil « Écoutés dans Seaglass ». */
export const PLAY_HISTORY_KEY = "SEAGLASS_RECENT";

/** Segment de biographie : texte ou lien interne (#205). */
export type BioPart =
  | { kind: "text"; text: string }
  | { kind: "artist"; text: string; id: number }
  | { kind: "album"; text: string; id: number };

/** Bas de la page d'album (#350), chargé après elle : chaque liste peut être vide. */
export interface AlbumRelated {
  versions: AlbumSummary[];
  byArtist: AlbumSummary[];
}

/** Compléments de la page d'artiste (#205), chargés après elle. */
export interface ArtistExtra {
  bio: { source: string | null; paragraphs: BioPart[][] } | null;
  similar: ArtistSummary[];
  appearsOn: AlbumSummary[];
}

export interface PlaylistPage {
  playlist: PlaylistSummary;
  /** Dans l'ordre de la playlist. */
  tracks: TrackEntry[];
  /** Version de la playlist lue (#152) : à rendre à `playlist` pour savoir si elle a changé. */
  revision: string | null;
}

/** Changement proposé par « Nettoyer » (#382) ; `index` = position dans la liste lue,
 *  `op` = ce que l'accepter fait (renvoyé tel quel). */
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
  /** Coché d'office (pas un indisponible sans remplaçant, dont le retrait est définitif). */
  suggested: boolean;
};

export interface CleanupReport {
  items: CleanupItem[];
  /** Titres lus. */
  scanned: number;
}

/** Modification d'une liste (#382) ; les favoris ignorent `index`. */
export type ListOp =
  | { op: "remove"; index: number; trackId: TrackId }
  | { op: "insert"; index: number; trackId: TrackId }
  | { op: "replace"; index: number; trackId: TrackId; with: TrackId };

export interface ListOpsResult {
  applied: number;
  /** Titres que le service refuse de rajouter (retirés du catalogue). */
  skipped: number;
  /** De quoi annuler, à rendre tel quel. */
  undo: ListOp[];
  error: string | null;
}

/** Paroles (`tidal/models.rs` : `Lyrics`). */
export interface Lyrics {
  trackId: TrackId;
  /** Fournisseur à créditer (« MUSIXMATCH »). */
  provider: string | null;
  /** Écriture de droite à gauche. */
  rtl: boolean;
  text: { kind: "synced"; lines: LyricLine[] } | { kind: "plain"; lines: string[] };
}

export interface LyricLine {
  /** Début de la ligne, en millisecondes ; texte vide = passage instrumental. */
  timeMs: number;
  text: string;
}

export interface MixPage {
  mix: MixSummary;
  /** Image 640×640 pour l'en-tête. */
  imageLargeUrl: string | null;
  tracks: Track[];
}

export type RadioSeed = "track" | "artist";

/** Radio d'un titre ou d'un artiste. */
export interface RadioPage {
  seed: RadioSeed;
  id: number;
  /** Titre du morceau ou nom de l'artiste de départ. */
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

/** Raccourci de la barre latérale (bureau) ou tuile du mode conduite (Android, #193) ; le titre
 *  d'une playlist ou d'un mix est gardé pour l'afficher sans requête. */
export type Pin =
  | { kind: "playlist"; uuid: string; title: string }
  | { kind: "mix"; id: string; title: string }
  | { kind: "likedTracks" }
  | { kind: "later" }
  | { kind: "smart"; list: SmartList };

/** Critères de tri d'une liste de titres (#1, #2). */
export type SortKey = "position" | "added" | "title" | "artist" | "album" | "duration";

export interface TrackSort {
  key: SortKey;
  descending: boolean;
}

/** Tri mémorisé d'une liste (#93) : uuid de playlist, ou `likedTracks`. */
export interface ListSort extends TrackSort {
  list: string;
}

/** Version du journal (#340), entrées déjà filtrées pour la plateforme. */
export interface Release {
  version: string;
  /** `AAAA-MM-JJ` pour une version publiée. */
  published: string | null;
  entries: string[];
}

export interface UiSettings {
  theme: Theme;
  home: HomeLayout;
  pins: Pin[];
  /** Couleur d'accent `#rrggbb` de chaque thème (#250) ; thème absent : celle de son CSS. */
  accents: Partial<Record<Theme, string>>;
  /** Familles de thèmes favorites (#390), en tête de la galerie ; voir `lib/themes.ts`. */
  favoriteThemes: Theme[];
  /** Langue choisie (#7) ; `system` suit la langue d'affichage de Windows. */
  language: LanguagePref;
  /** Fermer la fenêtre : quitter, ou réduire dans la zone de notification (#83). */
  closeAction: CloseAction;
  /** Présence Discord (#215), désactivée par défaut. */
  discord: boolean;
  /** Tris mémorisés, le plus récent en tête (#93). */
  sorts: ListSort[];
  /** Taille de l'interface en pourcentage (#337), une valeur de `ZOOM_LEVELS`. */
  zoom: number;
  /** Animations de l'interface (#351). */
  motion: Motion;
  /** Clins d'œil de l'interface (#399), activés par défaut ; voir `lib/eggs/`. */
  easterEggs: boolean;
  /** Contenu masqué dans ce que le service propose (#368) ; filtré côté Rust. */
  content: ContentFilter;
  /** Raccourcis clavier globaux (#413, bureau). */
  globalShortcuts: GlobalShortcuts;
  /** Vérification périodique des mises à jour (#452), activée par défaut. */
  autoUpdates: boolean;
  /** Jam (#452) : désactivée, elle disparaît de l'UI. */
  jam: boolean;
  /** Notification des nouvelles sorties des artistes suivis (#435), désactivée par défaut. */
  releaseNotifications: boolean;
}

/** Envoi des écoutes à ListenBrainz (#431). Le jeton n'est jamais renvoyé à l'UI. */
export interface ListenBrainzStatus {
  /** Compte connecté ; `null` : envoi désactivé. */
  user: string | null;
  /** ListenBrainz a refusé le jeton à un envoi : il faut en coller un nouveau. */
  refused: boolean;
  /** Écoutes pas encore envoyées. */
  pending: number;
}

/** Relais de la Jam en ligne et des transferts (#452). La clé n'est jamais renvoyée. */
export interface RelayView {
  enabled: boolean;
  /** Adresse choisie ; `null` : celle compilée (`defaultUrl`). */
  url: string | null;
  defaultUrl: string | null;
  /** Une clé propre à ce relais est enregistrée. */
  customKey: boolean;
  /** Adresse et clé connues : le relais est utilisable. */
  usable: boolean;
}

/** `url` vide : relais compilé. `key` absente : inchangée ; vide : clé compilée. */
export interface RelayChange {
  enabled: boolean;
  url: string | null;
  key?: string | null;
}

/** Action d'un raccourci global (#413). */
export type GlobalAction =
  "playPause" | "next" | "previous" | "volumeUp" | "volumeDown" | "like" | "showWindow";

/** Combinaisons écrites `Ctrl+Alt+Space` (modificateurs puis `KeyboardEvent.code`) ; action
 *  sans combinaison : absente. */
export interface GlobalShortcuts {
  enabled: boolean;
  bindings: Partial<Record<GlobalAction, string>>;
}

export interface GlobalShortcutsStatus {
  /** Le système offre des raccourcis globaux (faux sur Android, ou sans serveur X). */
  available: boolean;
  /** Combinaisons non enregistrées : prises par une autre app, ou touche inconnue. */
  failed: GlobalAction[];
}

/** Ce que le backend retire des listes proposées (pas des favoris ni de ses playlists). */
export interface ContentFilter {
  hideExplicit: boolean;
  /** Masqué par défaut (#369). */
  hideAi: boolean;
}

export type CloseAction = "quit" | "tray";

/** `system` suit « réduire les animations » du système. */
export type Motion = "system" | "reduced" | "full";

/** Langue effective de l'app, décidée par le backend. */
export type Lang = "fr" | "en";

/** Plateforme de l'app, décidée à la compilation côté Rust. */
export type Platform = "desktop" | "android";

export type LanguagePref = "system" | Lang;

/** Taille du cache mpv en Mio : seules ces valeurs sont acceptées par le backend. */
export type CacheSize = 32 | 64 | 128 | 256;

export type HiresUpgrade = "auto" | "ask" | "off";

/**
 * Qualité de lecture sur Android (#325, #366), paliers de TIDAL : AAC 96 et 320 kbit/s, FLAC
 * 16/44,1, FLAC hi-res jusqu'à 24/192.
 */
export type MobileQuality = "low" | "high" | "lossless" | "max";

/** Normalisation du volume par ReplayGain (#82). */
export type Normalization = "off" | "track" | "album";

/** Égaliseur paramétrique (#428). Bornes vérifiées côté Rust (`player::eq`). */
export type EqFilterKind = "peaking" | "lowShelf" | "highShelf";

export interface EqFilter {
  kind: EqFilterKind;
  /** Hz, 20 à 20 000. */
  freq: number;
  /** dB, ±20. */
  gain: number;
  /** 0,1 à 10. */
  q: number;
}

export interface EqProfile {
  id: number;
  name: string;
  /** dB ; `null` = automatique (compense le plus fort gain positif). */
  preamp: number | null;
  filters: EqFilter[];
}

export interface EqSettings {
  /** Désactivé par défaut. */
  enabled: boolean;
  active: number | null;
  profiles: EqProfile[];
}

/** Réglages de lecture, stockés à part des réglages d'UI. */
export interface PlaybackSettings {
  cacheSize: CacheSize;
  /** Édition hi-res d'un titre qui n'en a pas (même enregistrement) : la lire à sa place,
   *  la proposer (#59) ou l'ignorer. */
  hiresUpgrade: HiresUpgrade;
  /** Désactivée par défaut : seul mode qui laisse le signal intact. */
  normalization: Normalization;
  equalizer: EqSettings;
  /** Périphérique de sortie choisi (#79) ; `null` = périphérique par défaut du système.
   *  Gardé même débranché : le lecteur se replie sur le défaut puis y revient. */
  audioDevice: AudioDevice | null;
  /** Mode exclusif WASAPI (#245), désactivé par défaut : normalisation suspendue, les
   *  autres apps n'ont plus de son pendant la lecture. Sans effet sur Android. */
  exclusive: boolean;
  /** Fin de file : enchaîner sur la radio du dernier titre (#78). */
  autoRadio: boolean;
  /** Bureau : pause quand le périphérique de sortie utilisé est débranché (#348). Activé
   *  par défaut. */
  pauseOnUnplug: boolean;
  /** Bureau : reprendre la lecture au lancement de l'app (#353). Désactivé par défaut. */
  resumeOnLaunch: boolean;
  /** Bureau : volume, normalisation et mode exclusif retenus par périphérique de sortie
   *  (#416). Désactivé par défaut. */
  deviceProfiles: boolean;
  /** Android : qualité demandée (#325), Lossless par défaut. Sans effet sur le bureau. */
  /** Android (#365) : réseau non facturé (Wi-Fi) et facturé (données mobiles). */
  wifiQuality: MobileQuality;
  cellularQuality: MobileQuality;
  /** Android (#365) : plafond quand la batterie est faible. */
  lowBatteryQuality: boolean;
}

/** Android (#492) : qualité baissée par la batterie en ce moment. */
export interface QualityCap {
  /** Qualité demandée pour l'instant. */
  quality: MobileQuality;
  /** Celle du réglage pour le réseau en cours. */
  chosen: MobileQuality;
  reason: "powerSave" | "batteryLow";
  /** Niveau en %, s'il est connu. */
  battery: number | null;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
  /** Android seulement : taille de l'APK en octets, proposé avant d'être téléchargé. */
  size?: number;
  /** Bureau : installeur en cours de téléchargement, `update-ready` suivra (#187). */
  downloading?: boolean;
}

export interface SessionInfo {
  userId: number;
  countryCode: string;
}

export interface Toast {
  level: "error" | "info";
  message: string;
  /** Refus du lecteur (clin d'œil #399), que l'UI peut habiller selon le thème. */
  kind?: "unplayable" | "stopped";
}

/** Jam (#25) : en ligne par le relais, ou sur le réseau local. */
export type JamMode = "online" | "local";
export type JamConnection = "connecting" | "online" | "reconnecting" | "failed";

/** Serveur de la télécommande (#336) en cours, bureau seulement : télécommande et / ou
 *  transferts de lecture (#414). */
export interface RemoteStatus {
  /** Page et commandes de la télécommande servies. */
  remote: boolean;
  /** Transferts de lecture acceptés depuis l'app Android. */
  handoff: boolean;
  url: string;
  /** Code de propriétaire, 12 caractères. */
  code: string;
  /** Faux si aucune adresse du réseau local n'a été trouvée. */
  lan: boolean;
  qrSvg: string | null;
}

/** Ordinateur lié pour les transferts de lecture (#414), côté téléphone. */
export interface LinkedComputer {
  /** `192.168.1.105:47801`. */
  host: string;
  /** Dernière synchro réussie de l'historique d'écoute (ms, #430). */
  syncedAt: number | null;
  /** Joignable aussi hors du réseau local, par le relais (#430). */
  online: boolean;
}

/** Jam (#25) : adresse et code à donner aux invités. */
export interface JamStatus {
  mode: JamMode;
  /** `null` tant que le relais n'a pas répondu. */
  url: string | null;
  code: string;
  /** Réseau local : faux si aucune adresse du réseau n'a été trouvée (`127.0.0.1`). */
  lan: boolean;
  /** QR code de l'adresse avec le code, en SVG noir sur blanc (généré par le backend). */
  qrSvg: string | null;
  connection: JamConnection;
  /** Raison d'une reconnexion ou d'un échec, déjà traduite. */
  reason: string | null;
  /** Nom de l'invité de chaque titre ajouté par la Jam, par identifiant de titre. */
  guests: Record<string, string>;
  /** Règles de l'hôte (#222, #288), retenues d'une Jam à l'autre. */
  rules: JamRules;
  /** Les invités peuvent ajouter (suspension propre à la Jam, #288). */
  addsOpen: boolean;
  /** Invités qui ont ajouté pendant la Jam, exclus compris (#288). */
  members: JamGuest[];
}

/** Règles de l'hôte pour la Jam (#288). */
export interface JamRules {
  /** Les invités peuvent mettre en pause et relancer (#222). */
  guestControls: boolean;
  /** Titres en attente au plus par invité ; `null` = sans limite. */
  maxPending: number | null;
  /** Les invités peuvent ajouter un album entier. */
  albums: boolean;
  /** Les invités peuvent régler le volume (#449). */
  guestVolume: boolean;
  /** Volume le plus haut qu'un invité peut demander (1 à 100). */
  guestVolumeMax: number;
  /** Les invités peuvent voter pour passer le titre en cours (#433). */
  skipVote: boolean;
  /** Votes qui passent le titre (2 à 10). */
  skipVotes: number;
}

/** Invité de la Jam vu par l'hôte (#288). */
export interface JamGuest {
  id: number;
  name: string | null;
  /** Titres ajoutés pendant la Jam. */
  added: number;
  banned: boolean;
}

/** Événement `jam-guests` : noms des titres et invités. */
export interface JamGuestsUpdate {
  guests: Record<string, string>;
  members: JamGuest[];
}

export const commands = {
  authStatus: () => invoke<SessionInfo | null>("auth_status"),
  login: () => invoke<SessionInfo>("login"),
  /** Vrai si la connexion passe par le navigateur du système (Linux, #17). */
  loginInBrowser: () => invoke<boolean>("login_in_browser"),
  /** Ouvre la page de connexion dans le navigateur du système et renvoie son adresse. */
  browserLoginStart: () => invoke<string>("browser_login_start"),
  /** Adresse de la page sur laquelle TIDAL a renvoyé après la connexion. */
  browserLoginFinish: (redirect: string) =>
    invoke<SessionInfo>("browser_login_finish", { redirect }),
  logout: () => invoke<void>("logout"),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  /** Dernières recherches retenues, la plus récente en tête (#211). */
  searchHistory: () => invoke<string[]>("search_history"),
  /** Garde une requête dans l'historique (#248 : `search` ne le fait plus). */
  noteSearchHistory: (query: string) => invoke<string[]>("note_search_history", { query }),
  removeSearchHistory: (query: string) => invoke<string[]>("remove_search_history", { query }),
  clearSearchHistory: () => invoke<string[]>("clear_search_history"),
  /** Historique d'écoute local (#202) : albums récents et les plus écoutés. */
  playHistory: () => invoke<PlayHistory>("play_history"),
  clearPlayHistory: () => invoke<void>("clear_play_history"),
  playStats: (period: StatsPeriod) => invoke<PlayStats>("play_stats", { period }),
  recapPeriods: () => invoke<RecapPeriod[]>("recap_periods"),
  recap: (year: number, month: number | null) => invoke<Recap>("recap", { year, month }),
  smartList: (list: SmartList) => invoke<Track[]>("smart_list", { list }),
  /** Jam locale (#25), bureau seulement : `null` si aucune Jam ne tourne. */
  jamStatus: () => invoke<JamStatus | null>("jam_status"),
  jamOnlineAvailable: () => invoke<boolean>("jam_online_available"),
  /** Relais (#452) : l'adresse choisie est vérifiée (`/healthz`) avant d'être enregistrée. */
  listenbrainzStatus: () => invoke<ListenBrainzStatus>("listenbrainz_status"),
  listenbrainzConnect: (token: string) =>
    invoke<ListenBrainzStatus>("listenbrainz_connect", { token }),
  listenbrainzDisconnect: () => invoke<ListenBrainzStatus>("listenbrainz_disconnect"),
  relaySettings: () => invoke<RelayView>("relay_settings"),
  setRelaySettings: (change: RelayChange) => invoke<RelayView>("set_relay_settings", { change }),
  /** Dossier des mises à jour compilé (#452) ; `null` : mises à jour absentes. */
  updateServer: () => invoke<string | null>("update_server"),
  /** En ligne, l'adresse arrive ensuite par `jam-status`. */
  startJam: (mode: JamMode) => invoke<JamStatus>("start_jam", { mode }),
  stopJam: () => invoke<void>("stop_jam"),
  /** Règles de l'hôte, enregistrées et appliquées (#222, #288) ; `null` sans Jam. */
  setJamRules: (rules: JamRules) => invoke<JamStatus | null>("set_jam_rules", { rules }),
  /** Suspend (`false`) ou rouvre les ajouts des invités (#288). */
  setJamAddsOpen: (open: boolean) => invoke<JamStatus | null>("set_jam_adds_open", { open }),
  /** Exclut un invité (ses titres en attente sont retirés) ou le réadmet (#288). */
  setJamGuestBanned: (id: number, banned: boolean) =>
    invoke<JamStatus | null>("set_jam_guest_banned", { id, banned }),
  /** Télécommande (#336) : `null` si elle est éteinte. */
  remoteStatus: () => invoke<RemoteStatus | null>("remote_status"),
  /** Allume la télécommande (choix et code retenus). */
  startRemote: () => invoke<RemoteStatus | null>("start_remote"),
  stopRemote: () => invoke<RemoteStatus | null>("stop_remote"),
  /** #414 : « Accepter les transferts » ; `null` si le serveur s'arrête. */
  setHandoffAccepted: (enabled: boolean) =>
    invoke<RemoteStatus | null>("set_handoff_accepted", { enabled }),
  /** #414, téléphone : ordinateur lié (jamais le code). */
  linkedComputer: () => invoke<LinkedComputer | null>("linked_computer"),
  /** Adresse de la télécommande du PC avec son code après `#`, vérifiée avant d'être gardée. */
  linkComputer: (link: string) => invoke<LinkedComputer>("link_computer", { link }),
  unlinkComputer: () => invoke<void>("unlink_computer"),
  /** « Reprendre ici » : la lecture du PC passe sur le téléphone, le PC se met en pause. */
  handoffPull: () => invoke<void>("handoff_pull"),
  /** « Continuer sur l'ordinateur » : le téléphone se met en pause, le PC reprend. */
  handoffPush: () => invoke<void>("handoff_push"),
  /** #430 : historique d'écoute échangé avec l'ordinateur lié ; date de la dernière synchro
   *  réussie, `null` sans ordinateur lié. Sans `force`, rien si elle est toute récente. */
  syncHistory: (force: boolean) => invoke<number | null>("sync_history", { force }),
  /** Nouveau code : les téléphones appairés sont déconnectés. */
  renewRemoteCode: () => invoke<RemoteStatus | null>("renew_remote_code"),
  album: (id: number) => invoke<AlbumPage>("album", { id }),
  artist: (id: number) => invoke<ArtistPage>("artist", { id }),
  /** Autres versions et autres albums de l'artiste (#350), album affiché exclu. */
  albumRelated: (id: number) => invoke<AlbumRelated>("album_related", { id }),
  /** Biographie, similaires, « Apparaît sur » (#205) : chaque partie peut être vide. */
  artistExtra: (id: number) => invoke<ArtistExtra>("artist_extra", { id }),
  /** `null` si la playlist n'a pas changé depuis `revision` (#152). */
  playlist: (uuid: string, revision: string | null = null) =>
    invoke<PlaylistPage | null>("playlist", { uuid, revision }),
  /** Copie gardée par le backend, sans réseau : affichée en attendant `playlist`. */
  cachedPlaylist: (uuid: string) => invoke<PlaylistPage | null>("cached_playlist", { uuid }),
  /** Copie de l'onglet enregistrée au dernier lancement (#181), sans réseau ; pas de mixes. */
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
  /** Tout l'accueil, pages suivantes comprises : remplace la première page. */
  homeRest: () => invoke<HomeSection[]>("home_rest"),
  /** Fil complété des sorties des artistes suivis (#415) ; émet aussi `feed-unseen`. */
  feed: () => invoke<FeedItem[]>("feed"),
  /** Marque comme vu tout le dernier fil lu (ouverture de la page). */
  markFeedSeen: () => invoke<void>("mark_feed_seen"),
  /** `null` si TIDAL n'a pas de paroles pour ce titre. */
  lyrics: (trackId: TrackId) => invoke<Lyrics | null>("lyrics", { trackId }),
  /** Format réel, ReplayGain, BPM… : une requête de lecture, à ne demander qu'à la demande. */
  trackInfo: (trackId: TrackId) => invoke<TrackInfo>("track_info", { trackId }),
  /** Lien TIDAL contenu dans un texte collé, ou `null` (un titre est résolu en son album). */
  openLink: (text: string) => invoke<LinkTarget | null>("open_link", { text }),
  /** Lit une playlist d'un autre service et cherche ses titres (#328) ; avancement par
   *  `onImportProgress`. Rien n'est enregistré. */
  importPlaylist: (link: string) => invoke<ImportPreview>("import_playlist", { link }),
  /** Arrête l'import en cours (page quittée). */
  cancelImport: () => invoke<void>("cancel_import"),
  /** Crédits de tout l'album, à la demande (une requête par page de 100 titres). */
  albumCredits: (albumId: number) => invoke<AlbumCreditGroup[]>("album_credits", { albumId }),
  /** Lot de titres d'un contributeur (#429) : `offset` = `nextOffset` du lot précédent, 0 au départ. */
  contributor: (artistId: number, offset: number) =>
    invoke<ContributorPage>("contributor", { artistId, offset }),
  /** Album hi-res au même titre, pour un album qui ne l'est pas (#59). */
  hiresTwinAlbum: (albumId: number) => invoke<AlbumSummary | null>("hires_twin_album", { albumId }),
  /** Identifiants de tous les favoris (mis à jour ensuite par l'événement `likes`). */
  likes: () => invoke<LikedIds>("likes"),
  /** Ajoute aux favoris ou retire ; l'événement `likes` suit la confirmation. */
  setLiked: (target: LikeTarget, liked: boolean) => invoke<void>("set_liked", { target, liked }),
  /** Plusieurs titres ajoutés aux favoris ou retirés en une fois (sélection, #332). */
  setTracksLiked: (trackIds: TrackId[], liked: boolean) =>
    invoke<void>("set_tracks_liked", { trackIds, liked }),
  /** Titres et artistes bloqués (#334), lus chez le service. */
  blocked: () => invoke<Blocked>("blocked"),
  /** Bloque ou débloque un titre ou un artiste. */
  setBlocked: (target: BlockTarget, blocked: boolean) =>
    invoke<void>("set_blocked", { target, blocked }),
  /** Identifiants de « Plus tard » (#196), mis à jour ensuite par l'événement `later`. */
  laterIds: () => invoke<LikedIds>("later_ids"),
  /** Toute la liste « Plus tard », du plus récent au plus ancien. */
  saveForLater: () => invoke<LibraryList<LaterItem>>("save_for_later"),
  /** Ajoute à « Plus tard » ou retire (pas de mix) ; l'événement `later` suit. */
  setSavedForLater: (target: LikeTarget, saved: boolean) =>
    invoke<void>("set_saved_for_later", { target, saved }),
  /** Playlists créées par l'utilisateur, les seules modifiables. */
  ownPlaylists: () => invoke<PlaylistSummary[]>("own_playlists"),
  /** Ajoute à la fin, sans doublon ; renvoie le nombre de titres réellement ajoutés. */
  addToPlaylist: (uuid: string, trackIds: TrackId[]) =>
    invoke<number>("add_to_playlist", { uuid, trackIds }),
  /** Crée une playlist avec ces titres ; renvoie son uuid. */
  createPlaylist: (title: string, trackIds: TrackId[]) =>
    invoke<string>("create_playlist", { title, trackIds }),
  /** `index` : position dans la playlist (0 = premier) ; refusé si un autre titre s'y trouve. */
  removeFromPlaylist: (uuid: string, index: number, trackId: TrackId) =>
    invoke<void>("remove_from_playlist", { uuid, index, trackId }),
  /** Retire plusieurs positions (#332) ; tout est refusé si l'une a changé. */
  removeManyFromPlaylist: (uuid: string, items: PlaylistItem[]) =>
    invoke<void>("remove_many_from_playlist", { uuid, items }),
  /** Annule un retrait par lot ; renvoie le nombre de titres remis. */
  restoreManyToPlaylist: (uuid: string, items: PlaylistItem[]) =>
    invoke<number>("restore_many_to_playlist", { uuid, items }),
  /** Remet un titre retiré à sa position (annulation, #349) ; `false` si rien n'a été ajouté. */
  restoreToPlaylist: (uuid: string, index: number, trackId: TrackId) =>
    invoke<boolean>("restore_to_playlist", { uuid, index, trackId }),
  /** Déplace le titre de `from` à `to` (position finale, 0 = premier, #201). */
  moveInPlaylist: (uuid: string, from: number, to: number, trackId: TrackId) =>
    invoke<void>("move_in_playlist", { uuid, from, to, trackId }),
  /** Renomme et change la description (#201). */
  editPlaylist: (uuid: string, title: string, description: string) =>
    invoke<void>("edit_playlist", { uuid, title, description }),
  deletePlaylist: (uuid: string) => invoke<void>("delete_playlist", { uuid }),
  /** Dossiers de playlists (#331), triés par nom ; vide si l'utilisateur n'en a pas. */
  playlistFolders: () => invoke<PlaylistFolder[]>("playlist_folders"),
  /** Crée un dossier à la racine, avec ces playlists dedans. */
  createFolder: (name: string, uuids: string[]) =>
    invoke<PlaylistFolder>("create_folder", { name, uuids }),
  renameFolder: (id: string, name: string) => invoke<void>("rename_folder", { id, name }),
  /** Range des playlists dans un dossier, ou les ressort à la racine (`folderId` nul). */
  moveToFolder: (uuids: string[], folderId: string | null) =>
    invoke<void>("move_to_folder", { uuids, folderId }),
  /** Supprime un dossier ; ses playlists sont gardées, à la racine. */
  deleteFolder: (id: string) => invoke<void>("delete_folder", { id }),
  /** Doublons et titres indisponibles d'une de vos playlists (#382). */
  playlistCleanup: (uuid: string) => invoke<CleanupReport>("playlist_cleanup", { uuid }),
  /** Applique les changements choisis (positions du rapport, dans n'importe quel ordre). */
  cleanPlaylist: (uuid: string, ops: ListOp[]) =>
    invoke<ListOpsResult>("clean_playlist", { uuid, ops }),
  /** Annule : `ops` = le `undo` du nettoyage. */
  undoPlaylistCleanup: (uuid: string, ops: ListOp[]) =>
    invoke<ListOpsResult>("undo_playlist_cleanup", { uuid, ops }),
  /** Doublons et titres indisponibles des titres likés (#382). */
  likedCleanup: () => invoke<CleanupReport>("liked_cleanup"),
  /** Applique des changements aux titres likés, ou les annule ; l'événement `likes` suit. */
  cleanLiked: (ops: ListOp[]) => invoke<ListOpsResult>("clean_liked", { ops }),
  /** Blind test (#434) : partie de `rounds` manches tirées dans `trackIds`. */
  /** Avec `jam`, les invités de la Jam en cours jouent aussi (#441). */
  blindTestStart: (
    trackIds: TrackId[],
    mode: BlindMode,
    rounds: number,
    title: string,
    jam = false,
    hostPlays = true,
  ) => invoke<void>("blind_test_start", { trackIds, mode, rounds, title, jam, hostPlays }),
  /** Révèle la manche sans attendre les réponses qui manquent (à plusieurs). */
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
  /** Android : vrai une fois après le lien `seaglass://drive` ou le raccourci de l'app (#193). */
  takeDriveRequest: () => invoke<boolean>("take_drive_request"),
  /** Android : épingle demandée par un raccourci de l'icône ou le lien `seaglass://pin/…`
   *  (#335), une seule fois. */
  takePinRequest: () => invoke<Pin | null>("take_pin_request"),
  /** Android : écran gardé allumé (mode conduite, #193) ; rien sur le bureau. */
  keepScreenOn: (on: boolean) => invoke<void>("keep_screen_on", { on }),
  playNext: (trackId: TrackId) => invoke<void>("play_next", { trackId }),
  addToQueue: (trackId: TrackId) => invoke<void>("add_to_queue", { trackId }),
  /** Plusieurs titres dans l'ordre : en tête de la file avec `next` (« Lire ensuite »), sinon à la fin. */
  enqueueTracks: (trackIds: TrackId[], next: boolean) =>
    invoke<void>("enqueue_tracks", { trackIds, next }),
  removeFromQueue: (index: number) => invoke<void>("remove_from_queue", { index }),
  /** Déplace l'entrée `from` de la file manuelle pour qu'elle finisse à l'index `to`. */
  moveInQueue: (from: number, to: number) => invoke<void>("move_in_queue", { from, to }),
  clearQueue: () => invoke<void>("clear_queue"),
  /** Annule le dernier vidage ou retrait d'un titre de la file (#349) ; `false` si la
   *  file a changé depuis. */
  restoreQueue: () => invoke<boolean>("restore_queue"),
  /** Lit le titre `index` de `QueueView.manual` ou `.upcoming` (#167) ; sans effet si ce
   *  n'est plus `trackId` (file changée depuis l'affichage). */
  playFromQueue: (part: QueuePart, index: number, trackId: TrackId) =>
    invoke<void>("play_from_queue", { part, index, trackId }),
  /** Réponse au bandeau « Édition hi-res disponible », valable pour tout l'album (#59). */
  answerHiresOffer: (accept: boolean) => invoke<void>("answer_hires_offer", { accept }),
  /** Pause après une durée ou à la fin du titre en cours ; `null` annule (#85). */
  setSleepTimer: (timer: SleepRequest | null) => invoke<void>("set_sleep_timer", { timer }),
  next: () => invoke<void>("next"),
  previous: () => invoke<void>("previous"),
  togglePause: () => invoke<void>("toggle_pause"),
  seek: (seconds: number) => invoke<void>("seek", { seconds }),
  setShuffle: (enabled: boolean) => invoke<void>("set_shuffle", { enabled }),
  setRepeat: (mode: Repeat) => invoke<void>("set_repeat", { mode }),
  /** 0 à 100 ; rétablit le son s'il était coupé (sauf à 0). */
  setVolume: (volume: number) => invoke<void>("set_volume", { volume }),
  setMuted: (muted: boolean) => invoke<void>("set_muted", { muted }),
  playerState: () => invoke<PlayerState>("player_state"),
  queueState: () => invoke<QueueView>("queue_state"),
  queueTrackIds: () => invoke<number[]>("queue_track_ids"),
  uiSettings: () => invoke<UiSettings>("ui_settings"),
  setUiSettings: (settings: UiSettings) => invoke<void>("set_ui_settings", { settings }),
  globalShortcutsStatus: () => invoke<GlobalShortcutsStatus>("global_shortcuts_status"),
  /** Langue effective (réglage, ou langue du système). */
  language: () => invoke<Lang>("language"),
  /** Enregistre le réglage ; renvoie la langue effective. */
  setLanguage: (language: LanguagePref) => invoke<Lang>("set_language", { language }),
  /** Sections affichées, dans l'ordre de l'écran ; renvoie la disposition enregistrée. */
  setHomeLayout: (sections: { key: string; hidden: boolean }[]) =>
    invoke<HomeLayout>("set_home_layout", { sections }),
  /** Affiche ou masque le bandeau « À la une » (#185) ; renvoie la disposition enregistrée. */
  setHomeFeatured: (shown: boolean) => invoke<HomeLayout>("set_home_featured", { shown }),
  resetHomeLayout: () => invoke<HomeLayout>("reset_home_layout"),
  /** Opens the website, the repository or a new issue in the browser (#188, #517); fixed URLs on the Rust side. */
  openProjectPage: (page: ProjectPage) => invoke<void>("open_project_page", { page }),
  /** #192 : vrai une seule fois si le lancement précédent a planté. */
  takeCrashNotice: () => invoke<boolean>("take_crash_notice"),
  /** Rapport du dernier plantage, à copier dans une issue. */
  lastCrashReport: () => invoke<string | null>("last_crash_report"),
  /** Fin du journal de ce lancement. */
  currentLog: () => invoke<string | null>("current_log"),
  /** #352 : réglages de cet appareil en JSON, à coller ailleurs. */
  exportSettings: () => invoke<string>("export_settings"),
  /** Reprend une sauvegarde collée ; clés `ui.theme`, `playback.autoRadio`… */
  importSettings: (text: string) => invoke<ImportReport>("import_settings", { text }),
  changelog: () => invoke<Release[]>("changelog"),
  /** Bureau : dossier des journaux dans l'explorateur. */
  openLogDir: () => invoke<void>("open_log_dir"),
  /** Bureau (#330) : exporte la bibliothèque en JSON et CSV dans Documents\Seaglass. */
  exportLibrary: () => invoke<LibraryExportReport>("export_library"),
  cancelLibraryExport: () => invoke<void>("cancel_library_export"),
  /** Remplace les épingles ; renvoie la liste enregistrée (validée, sans doublons). */
  setPins: (pins: Pin[]) => invoke<Pin[]>("set_pins", { pins }),
  /** Mémorise le tri d'une liste (`null` : l'oublie) ; renvoie les tris enregistrés. */
  setListSort: (list: string, sort: TrackSort | null) =>
    invoke<ListSort[]>("set_list_sort", { list, sort }),
  playbackSettings: () => invoke<PlaybackSettings>("playback_settings"),
  /** Enregistre et applique à chaud (sans recharger le titre en cours). */
  setPlaybackSettings: (settings: PlaybackSettings) =>
    invoke<void>("set_playback_settings", { settings }),
  /** Android (#492) : plafond de batterie de l'instant ; `null` si la batterie ne change rien. */
  qualityCap: () => invoke<QualityCap | null>("quality_cap"),
  /** #428 : analyse un « ParametricEQ.txt » d'AutoEQ ; profil d'identifiant 0, sans nom. */
  parseAutoeq: (text: string) => invoke<EqProfile>("parse_autoeq", { text }),
  /** Périphériques de sortie connus de mpv ; la suite arrive par `audio-devices`. */
  audioDevices: () => invoke<AudioDevices>("audio_devices"),
  /** Plateforme de l'app : choisit la mise en page (tactile sur Android). */
  platform: () => invoke<Platform>("platform"),
  /** État réel du lancement au démarrage, lu dans le système. */
  autostartEnabled: () => invoke<boolean>("autostart_enabled"),
  /** Vrai si la présence Discord peut être proposée (bureau, application Discord compilée, #215). */
  discordAvailable: () => invoke<boolean>("discord_available"),
  /** Vrai si le mode exclusif peut être proposé (Windows seulement, #245, #17). */
  exclusiveAvailable: () => invoke<boolean>("exclusive_available"),
  /** Renvoie l'état relu après changement. */
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  /** Bureau seulement : plein écran de la fenêtre principale (#131). */
  /** Android : vrai une fois après un appui sur la notification (#163). */
  takeNowPlayingRequest: () => invoke<boolean>("take_now_playing_request"),
  /** Android : lien reçu par le menu « Partager » (#203), une seule fois. */
  takeSharedLink: () => invoke<LinkTarget | null>("take_shared_link"),
  /** Android 12+ : liens `https://tidal.com/…` donnés à l'app ; `null` = rien à régler (#203). */
  linkHandling: () => invoke<{ enabled: boolean } | null>("link_handling"),
  /** Android : ouvre le réglage « Ouvrir par défaut » de l'app. */
  openLinkSettings: () => invoke<void>("open_link_settings"),
  fullscreen: () => invoke<boolean>("fullscreen"),
  /** Renvoie l'état relu ; la suite arrive aussi par `fullscreen`. */
  setFullscreen: (fullscreen: boolean) => invoke<boolean>("set_fullscreen", { fullscreen }),
  /** Mini-lecteur du bureau (#384) : l'ouvrir cache la fenêtre principale, le fermer la réaffiche. */
  openMiniPlayer: () => invoke<void>("open_mini_player"),
  closeMiniPlayer: () => invoke<void>("close_mini_player"),
  miniPlayerOnTop: () => invoke<boolean>("mini_player_on_top"),
  /** Renvoie l'état retenu. */
  setMiniPlayerOnTop: (onTop: boolean) => invoke<boolean>("set_mini_player_on_top", { onTop }),
  updateStatus: () => invoke<UpdateInfo | null>("update_status"),
  /** Faux sous Linux hors d'une AppImage : mises à jour à faire à la main (#314). */
  updatesAvailable: () => invoke<boolean>("updates_available"),
  /** L'installeur ferme l'app puis la relance (Linux : AppImage remplacée, puis relance). */
  installUpdate: () => invoke<void>("install_update"),
  /** Vérifie tout de suite : la mise à jour prête (réémise en `update-ready`), en
   *  téléchargement (`downloading`, `update-ready` suivra), ou `null` si à jour. */
  checkForUpdates: () => invoke<UpdateInfo | null>("check_for_updates"),
};

export const events = {
  /** #430 : écoutes reçues de l'autre appareil (nombre) : recharger l'historique affiché. */
  onHistorySynced: (cb: (added: number) => void): Promise<UnlistenFn> =>
    listen<number>("history-synced", (e) => cb(e.payload)),
  /** Jam (#25) : connexion au relais (connecté, reconnexion, nouvelle adresse). */
  onJamStatus: (cb: (status: JamStatus) => void): Promise<UnlistenFn> =>
    listen<JamStatus>("jam-status", (e) => cb(e.payload)),
  /** Jam (#25) : un invité a ajouté ou retiré des titres, ou a été exclu (#288). */
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
  /** Bureau : le téléchargement de l'installeur a échoué (message de l'erreur, #187). */
  onUpdateFailed: (cb: (message: string) => void): Promise<UnlistenFn> =>
    listen<string>("update-failed", (e) => cb(e.payload)),
  /** #330 : progression de l'export de la bibliothèque. */
  /** Import d'une playlist (#328) : titres cherchés sur TIDAL. */
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
  /** Réglages de lecture changés par le backend (profil d'un périphérique de sortie, #416) :
   *  à relire. */
  onPlaybackSettings: (cb: () => void): Promise<UnlistenFn> =>
    listen<null>("playback-settings", () => cb()),
  /** Android : appui sur la notification (#163) ; la demande se prend par `takeNowPlayingRequest`. */
  onOpenNowPlaying: (cb: () => void): Promise<UnlistenFn> =>
    listen<null>("open-now-playing", () => cb()),
  /** Android : mode conduite demandé (#193) ; la demande se prend par `takeDriveRequest`. */
  onOpenDrive: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-drive", () => cb()),
  /** Android : épingle à lire (#335) ; elle se prend par `takePinRequest`. */
  onOpenPin: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-pin", () => cb()),
  /** Lien partagé prêt (Android, #203) ou notification touchée (#435) ; il se prend par
   * `takeSharedLink`. */
  onSharedLink: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-link", () => cb()),
  /** Raccourci global exécuté par l'UI (#413) : « Aimer » seulement. */
  onGlobalShortcut: (cb: (action: GlobalAction) => void): Promise<UnlistenFn> =>
    listen<GlobalAction>("global-shortcut", (e) => cb(e.payload)),
  onFullscreen: (cb: (fullscreen: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("fullscreen", (e) => cb(e.payload)),
  /** TIDAL a refusé le rafraîchissement en cours de route : reconnexion nécessaire (#259). */
  onSessionLost: (cb: () => void): Promise<UnlistenFn> => listen<null>("session-lost", () => cb()),
  /** Nombre de nouveautés du fil pas encore vues (#415). */
  onFeedUnseen: (cb: (count: number) => void): Promise<UnlistenFn> =>
    listen<number>("feed-unseen", (e) => cb(e.payload)),
  onToast: (cb: (t: Toast) => void): Promise<UnlistenFn> =>
    listen<Toast>("toast", (e) => cb(e.payload)),
};
