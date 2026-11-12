// Wrappers typés des commandes et événements Tauri.
// Types dupliqués depuis Rust : tidal/models.rs, player/state.rs, mpv/events.rs.

import { invoke, listen, type UnlistenFn } from "./transport";

export type TrackId = number;

/** Résultat d'un import de réglages (#352). */
export interface ImportReport {
  applied: string[];
  ignored: string[];
}

export interface ArtistRef {
  id: number;
  name: string;
}

export interface AlbumRef {
  id: number;
  title: string;
  /** Pochette 320×320. */
  coverUrl: string | null;
  /** 160×160 et 640×640 (#89). Toujours envoyées par le backend (déduites de la 320 pour un titre
   *  enregistré avant 0.4.36), absentes des anciennes fixtures : voir `coverSmall`. */
  coverSmallUrl?: string | null;
  coverLargeUrl?: string | null;
  /** Couleur dominante de la pochette (`#rrggbb`). */
  color: string | null;
}

export interface Track {
  id: TrackId;
  title: string;
  version: string | null;
  /** Secondes. */
  duration: number;
  artists: ArtistRef[];
  album: AlbumRef;
  explicit: boolean;
  /** Généré par IA selon le service (#369). */
  ai: boolean;
  hires: boolean;
  /** Identifiant de l'enregistrement, commun à ses éditions. */
  isrc?: string | null;
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

export type AudioQuality = "LOW" | "HIGH" | "LOSSLESS" | "HI_RES_LOSSLESS";

export interface StreamFormat {
  quality: AudioQuality;
  codec: string;
  bitDepth: number | null;
  sampleRate: number | null;
  /** Débit moyen déclaré par le manifeste, en kbps (#3). */
  bitrate: number | null;
  /** Album de l'édition hi-res réellement lue, quand ce n'est pas celle du titre affiché (#18). */
  hiresEdition: EditionAlbum | null;
}

/** Album d'une autre édition hi-res, ouvrable depuis « En lecture » (#111). */
export interface EditionAlbum {
  id: number;
  title: string;
}

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
  | { kind: "mix"; id: string };

/** Crédits d'un album entier (#206). */
export interface AlbumCreditGroup {
  role: string;
  contributors: AlbumContributor[];
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

export interface AudioParams {
  format: string;
  samplerate: number;
  channelCount: number;
}

export type Repeat = "off" | "all" | "one";

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

/** Sortie réellement utilisée, pour la chaîne audio (#79). */
export interface OutputDevice {
  /** Nom Windows du périphérique ouvert ; `null` si inconnu. */
  name: string | null;
  /** Périphérique par défaut du système (aucun choix, ou repli). */
  systemDefault: boolean;
  /** Périphérique choisi mais indisponible : nom enregistré. */
  missing: string | null;
}

export interface PlayerState {
  track: Track | null;
  stream: StreamFormat | null;
  output: AudioParams | null;
  /** Format réellement écrit sur la sortie (#245) : mixage de Windows en partagé, format
   *  ouvert sur le périphérique en exclusif. `null` sur Android. */
  deviceFormat: AudioParams | null;
  outputDriver: string | null;
  /** Mode exclusif WASAPI actif (#245). */
  exclusive: boolean;
  outputDevice: OutputDevice | null;
  loading: boolean;
  paused: boolean;
  buffering: boolean;
  idle: boolean;
  timePos: number;
  duration: number;
  shuffle: boolean;
  repeat: Repeat;
  hasPrevious: boolean;
  hasNext: boolean;
  volume: Volume;
  /** Édition hi-res proposée pour le titre courant (réglage « Demander », #59) : son format,
   *  l'album dans `hiresEdition`. */
  hiresOffer: StreamFormat | null;
  /** Gain de normalisation appliqué au titre courant, en dB (#82) ; `null` si le signal
   *  n'est pas modifié. */
  normalization: number | null;
  /** Minuteur de mise en veille actif (#85). */
  sleep: SleepState | null;
  /** mpv ne tourne pas, avec sa cause probable (#92) ; `null` quand tout va bien. */
  mpvIssue: MpvIssue | null;
}

/**
 * `cpuUnsupported` : mpv embarqué (x86-64-v3) sur un processeur sans AVX2.
 * `missing` : build installé sans son mpv (#261).
 */
export type MpvIssue = "stopped" | "cpuUnsupported" | "missing";

/** Minuteur actif : échéance en ms epoch (l'UI calcule le temps restant), ou fin du titre. */
export type SleepState = { kind: "minutes"; endsAt: number } | { kind: "endOfTrack" };

export type SleepRequest = { kind: "minutes"; minutes: number } | { kind: "endOfTrack" };

/** Volume logiciel de mpv : 100 non coupé = gain unitaire (seul réglage bit-perfect). */
export interface Volume {
  /** 0 à 100. */
  level: number;
  muted: boolean;
}

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
  | { kind: "likedTracks" }
  | { kind: "later" }
  | { kind: "search"; query: string }
  | { kind: "smart"; list: SmartList };

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
  /** Déjà vue dans un client TIDAL (lu seulement). */
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
  | { kind: "duplicate"; index: number; track: Track; keptIndex: number; kept: Track; sameTrack: boolean }
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
  text:
    | { kind: "synced"; lines: LyricLine[] }
    | { kind: "plain"; lines: string[] };
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

export type Theme = "seaglass" | "studio" | "atelier" | "acidwave" | "surreal" | "solarpunk" | "cyberpunk" | "neumorph" | "neumorph-dark" | "terminal" | "riso" | "bauhaus" | "aero" | "alloy" | "alloy-dark" | "midnight" | "estampe" | "artdeco" | "cassette" | "seance" | "onboard" | "noir";

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
  /** Contenu masqué dans ce que le service propose (#368) ; filtré côté Rust. */
  content: ContentFilter;
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

/** Réglages de lecture, stockés à part des réglages d'UI. */
export interface PlaybackSettings {
  cacheSize: CacheSize;
  /** Édition hi-res d'un titre qui n'en a pas (même enregistrement) : la lire à sa place,
   *  la proposer (#59) ou l'ignorer. */
  hiresUpgrade: HiresUpgrade;
  /** Désactivée par défaut : seul mode qui laisse le signal intact. */
  normalization: Normalization;
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
  /** Android : qualité demandée (#325), Lossless par défaut. Sans effet sur le bureau. */
  /** Android (#365) : réseau non facturé (Wi-Fi) et facturé (données mobiles). */
  wifiQuality: MobileQuality;
  cellularQuality: MobileQuality;
  /** Android (#365) : plafond quand la batterie est faible. */
  lowBatteryQuality: boolean;
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
}

/** Jam (#25) : en ligne par le relais, ou sur le réseau local. */
export type JamMode = "online" | "local";
export type JamConnection = "connecting" | "online" | "reconnecting" | "failed";

/** Télécommande allumée (#336), bureau seulement. */
export interface RemoteStatus {
  url: string;
  /** Code de propriétaire, 12 caractères. */
  code: string;
  /** Faux si aucune adresse du réseau local n'a été trouvée. */
  lan: boolean;
  qrSvg: string | null;
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
  browserLoginFinish: (redirect: string) => invoke<SessionInfo>("browser_login_finish", { redirect }),
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
  smartList: (list: SmartList) => invoke<Track[]>("smart_list", { list }),
  /** Jam locale (#25), bureau seulement : `null` si aucune Jam ne tourne. */
  jamStatus: () => invoke<JamStatus | null>("jam_status"),
  jamOnlineAvailable: () => invoke<boolean>("jam_online_available"),
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
  startRemote: () => invoke<RemoteStatus>("start_remote"),
  stopRemote: () => invoke<void>("stop_remote"),
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
  feed: () => invoke<FeedItem[]>("feed"),
  /** `null` si TIDAL n'a pas de paroles pour ce titre. */
  lyrics: (trackId: TrackId) => invoke<Lyrics | null>("lyrics", { trackId }),
  /** Format réel, ReplayGain, BPM… : une requête de lecture, à ne demander qu'à la demande. */
  trackInfo: (trackId: TrackId) => invoke<TrackInfo>("track_info", { trackId }),
  /** Lien TIDAL contenu dans un texte collé, ou `null` (un titre est résolu en son album). */
  openLink: (text: string) => invoke<LinkTarget | null>("open_link", { text }),
  /** Crédits de tout l'album, à la demande (une requête par page de 100 titres). */
  albumCredits: (albumId: number) => invoke<AlbumCreditGroup[]>("album_credits", { albumId }),
  /** Album hi-res au même titre, pour un album qui ne l'est pas (#59). */
  hiresTwinAlbum: (albumId: number) => invoke<AlbumSummary | null>("hires_twin_album", { albumId }),
  /** Identifiants de tous les favoris (mis à jour ensuite par l'événement `likes`). */
  likes: () => invoke<LikedIds>("likes"),
  /** Ajoute aux favoris ou retire ; l'événement `likes` suit la confirmation. */
  setLiked: (target: LikeTarget, liked: boolean) => invoke<void>("set_liked", { target, liked }),
  /** Titres et artistes bloqués (#334), lus chez le service. */
  blocked: () => invoke<Blocked>("blocked"),
  /** Bloque ou débloque un titre ou un artiste. */
  setBlocked: (target: BlockTarget, blocked: boolean) => invoke<void>("set_blocked", { target, blocked }),
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
  /** Doublons et titres indisponibles d'une de vos playlists (#382). */
  playlistCleanup: (uuid: string) => invoke<CleanupReport>("playlist_cleanup", { uuid }),
  /** Applique les changements choisis (positions du rapport, dans n'importe quel ordre). */
  cleanPlaylist: (uuid: string, ops: ListOp[]) => invoke<ListOpsResult>("clean_playlist", { uuid, ops }),
  /** Annule : `ops` = le `undo` du nettoyage. */
  undoPlaylistCleanup: (uuid: string, ops: ListOp[]) =>
    invoke<ListOpsResult>("undo_playlist_cleanup", { uuid, ops }),
  /** Doublons et titres indisponibles des titres likés (#382). */
  likedCleanup: () => invoke<CleanupReport>("liked_cleanup"),
  /** Applique des changements aux titres likés, ou les annule ; l'événement `likes` suit. */
  cleanLiked: (ops: ListOp[]) => invoke<ListOpsResult>("clean_liked", { ops }),
  /** Avec `shuffle`, active la lecture aléatoire et part d'un titre au hasard. */
  playTracks: (trackIds: TrackId[], startIndex: number, shuffle = false, source: PlaySource | null = null) =>
    invoke<void>("play_tracks", { trackIds, startIndex, shuffle, source }),
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
  /** Ouvre le dépôt ou une nouvelle issue dans le navigateur (#188) ; URLs fixes côté Rust. */
  openProjectPage: (page: "repo" | "newIssue") => invoke<void>("open_project_page", { page }),
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
  /** Remplace les épingles ; renvoie la liste enregistrée (validée, sans doublons). */
  setPins: (pins: Pin[]) => invoke<Pin[]>("set_pins", { pins }),
  /** Mémorise le tri d'une liste (`null` : l'oublie) ; renvoie les tris enregistrés. */
  setListSort: (list: string, sort: TrackSort | null) =>
    invoke<ListSort[]>("set_list_sort", { list, sort }),
  playbackSettings: () => invoke<PlaybackSettings>("playback_settings"),
  /** Enregistre et applique à chaud (sans recharger le titre en cours). */
  setPlaybackSettings: (settings: PlaybackSettings) =>
    invoke<void>("set_playback_settings", { settings }),
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
  /** Jam (#25) : connexion au relais (connecté, reconnexion, nouvelle adresse). */
  onJamStatus: (cb: (status: JamStatus) => void): Promise<UnlistenFn> =>
    listen<JamStatus>("jam-status", (e) => cb(e.payload)),
  /** Jam (#25) : un invité a ajouté ou retiré des titres, ou a été exclu (#288). */
  onJamGuests: (cb: (update: JamGuestsUpdate) => void): Promise<UnlistenFn> =>
    listen<JamGuestsUpdate>("jam-guests", (e) => cb(e.payload)),
  onPlayerState: (cb: (s: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("player-state", (e) => cb(e.payload)),
  onQueueState: (cb: (q: QueueView) => void): Promise<UnlistenFn> =>
    listen<QueueView>("queue-state", (e) => cb(e.payload)),
  onUpdateReady: (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>("update-ready", (e) => cb(e.payload)),
  /** Bureau : le téléchargement de l'installeur a échoué (message de l'erreur, #187). */
  onUpdateFailed: (cb: (message: string) => void): Promise<UnlistenFn> =>
    listen<string>("update-failed", (e) => cb(e.payload)),
  onLikes: (cb: (ids: LikedIds) => void): Promise<UnlistenFn> =>
    listen<LikedIds>("likes", (e) => cb(e.payload)),
  onLater: (cb: (ids: LikedIds) => void): Promise<UnlistenFn> =>
    listen<LikedIds>("later", (e) => cb(e.payload)),
  onAudioDevices: (cb: (d: AudioDevices) => void): Promise<UnlistenFn> =>
    listen<AudioDevices>("audio-devices", (e) => cb(e.payload)),
  /** Android : appui sur la notification (#163) ; la demande se prend par `takeNowPlayingRequest`. */
  onOpenNowPlaying: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-now-playing", () => cb()),
  /** Android : mode conduite demandé (#193) ; la demande se prend par `takeDriveRequest`. */
  onOpenDrive: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-drive", () => cb()),
  /** Android : épingle à lire (#335) ; elle se prend par `takePinRequest`. */
  onOpenPin: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-pin", () => cb()),
  /** Android : lien partagé prêt ; il se prend par `takeSharedLink` (#203). */
  onSharedLink: (cb: () => void): Promise<UnlistenFn> => listen<null>("open-link", () => cb()),
  onFullscreen: (cb: (fullscreen: boolean) => void): Promise<UnlistenFn> =>
    listen<boolean>("fullscreen", (e) => cb(e.payload)),
  /** TIDAL a refusé le rafraîchissement en cours de route : reconnexion nécessaire (#259). */
  onSessionLost: (cb: () => void): Promise<UnlistenFn> => listen<null>("session-lost", () => cb()),
  onToast: (cb: (t: Toast) => void): Promise<UnlistenFn> =>
    listen<Toast>("toast", (e) => cb(e.payload)),
};
