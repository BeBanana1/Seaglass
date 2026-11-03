// Wrappers typés des commandes et événements Tauri.
// Types dupliqués depuis Rust : tidal/models.rs, player/state.rs, mpv/events.rs.

import { invoke, listen, type UnlistenFn } from "./transport";

export type TrackId = number;

export interface ArtistRef {
  id: number;
  name: string;
}

export interface AlbumRef {
  id: number;
  title: string;
  coverUrl: string | null;
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
  hiresEdition: string | null;
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
  outputDriver: string | null;
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
  /** Minuteur de mise en veille actif (#85). */
  sleep: SleepState | null;
  /** mpv ne tourne pas, avec sa cause probable (#92) ; `null` quand tout va bien. */
  mpvIssue: MpvIssue | null;
}

/** `cpuUnsupported` : mpv embarqué (x86-64-v3) sur un processeur sans AVX2. */
export type MpvIssue = "stopped" | "cpuUnsupported";

/** Minuteur actif : échéance en ms epoch (l'UI calcule le temps restant), ou fin du titre. */
export type SleepState = { kind: "minutes"; endsAt: number } | { kind: "endOfTrack" };

export type SleepRequest = { kind: "minutes"; minutes: number } | { kind: "endOfTrack" };

/** Volume logiciel de mpv : 100 non coupé = gain unitaire (seul réglage bit-perfect). */
export interface Volume {
  /** 0 à 100. */
  level: number;
  muted: boolean;
}

export interface QueueView {
  manual: Track[];
  upcoming: Track[];
  /** Titre de départ quand la suite est une radio qui prolonge la file (#78). */
  radio: Track | null;
}

export interface AlbumSummary {
  id: number;
  title: string;
  artists: ArtistRef[];
  coverUrl: string | null;
  color: string | null;
  /** `AAAA-MM-JJ`. */
  releaseDate: string | null;
  numberOfTracks: number | null;
  /** `ALBUM`, `EP` ou `SINGLE`. */
  albumType: string | null;
  explicit: boolean;
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

export interface PlaylistPage {
  playlist: PlaylistSummary;
  /** Dans l'ordre de la playlist. */
  tracks: TrackEntry[];
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

export type Theme = "seaglass" | "studio" | "atelier" | "acidwave" | "surreal" | "solarpunk" | "cyberpunk" | "neumorph" | "neumorph-dark" | "terminal" | "riso" | "bauhaus";

/** Raccourci de la barre latérale ; le titre d'une playlist est gardé pour l'afficher sans requête. */
export type Pin = { kind: "playlist"; uuid: string; title: string } | { kind: "likedTracks" };

export interface UiSettings {
  theme: Theme;
  home: HomeLayout;
  pins: Pin[];
  /** Couleur d'accent `#rrggbb` ; `null` : celle du thème. */
  accent: string | null;
  /** Langue choisie (#7) ; `system` suit la langue d'affichage de Windows. */
  language: LanguagePref;
}

/** Langue effective de l'app, décidée par le backend. */
export type Lang = "fr" | "en";

export type LanguagePref = "system" | Lang;

/** Taille du cache mpv en Mio : seules ces valeurs sont acceptées par le backend. */
export type CacheSize = 32 | 64 | 128 | 256;

export type HiresUpgrade = "auto" | "ask" | "off";

/** Réglages de lecture, stockés à part des réglages d'UI. */
export interface PlaybackSettings {
  cacheSize: CacheSize;
  /** Édition hi-res d'un titre qui n'en a pas (même enregistrement) : la lire à sa place,
   *  la proposer (#59) ou l'ignorer. */
  hiresUpgrade: HiresUpgrade;
  /** Périphérique de sortie choisi (#79) ; `null` = périphérique par défaut du système.
   *  Gardé même débranché : le lecteur se replie sur le défaut puis y revient. */
  audioDevice: AudioDevice | null;
  /** Fin de file : enchaîner sur la radio du dernier titre (#78). */
  autoRadio: boolean;
}

export interface UpdateInfo {
  version: string;
  currentVersion: string;
  notes: string | null;
}

export interface SessionInfo {
  userId: number;
  countryCode: string;
}

export interface Toast {
  level: "error" | "info";
  message: string;
}

export const commands = {
  authStatus: () => invoke<SessionInfo | null>("auth_status"),
  login: () => invoke<SessionInfo>("login"),
  logout: () => invoke<void>("logout"),
  search: (query: string) => invoke<SearchResults>("search", { query }),
  album: (id: number) => invoke<AlbumPage>("album", { id }),
  artist: (id: number) => invoke<ArtistPage>("artist", { id }),
  playlist: (uuid: string) => invoke<PlaylistPage>("playlist", { uuid }),
  mix: (id: string) => invoke<MixPage>("mix", { id }),
  radio: (seed: RadioSeed, id: number) => invoke<RadioPage>("radio", { seed, id }),
  libraryMixes: () => invoke<MixSummary[]>("library_mixes"),
  libraryTracks: () => invoke<LibraryList<TrackEntry>>("library_tracks"),
  libraryAlbums: () => invoke<LibraryList<AlbumSummary>>("library_albums"),
  libraryArtists: () => invoke<LibraryList<ArtistSummary>>("library_artists"),
  libraryPlaylists: () => invoke<LibraryList<PlaylistSummary>>("library_playlists"),
  home: () => invoke<HomeSection[]>("home"),
  feed: () => invoke<FeedItem[]>("feed"),
  /** `null` si TIDAL n'a pas de paroles pour ce titre. */
  lyrics: (trackId: TrackId) => invoke<Lyrics | null>("lyrics", { trackId }),
  /** Format réel, ReplayGain, BPM… : une requête de lecture, à ne demander qu'à la demande. */
  trackInfo: (trackId: TrackId) => invoke<TrackInfo>("track_info", { trackId }),
  /** Album hi-res au même titre, pour un album qui ne l'est pas (#59). */
  hiresTwinAlbum: (albumId: number) => invoke<AlbumSummary | null>("hires_twin_album", { albumId }),
  /** Identifiants des titres likés (mis à jour ensuite par l'événement `liked-tracks`). */
  likedTracks: () => invoke<TrackId[]>("liked_tracks"),
  setTrackLiked: (trackId: TrackId, liked: boolean) =>
    invoke<void>("set_track_liked", { trackId, liked }),
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
  /** Avec `shuffle`, active la lecture aléatoire et part d'un titre au hasard. */
  playTracks: (trackIds: TrackId[], startIndex: number, shuffle = false) =>
    invoke<void>("play_tracks", { trackIds, startIndex, shuffle }),
  playNext: (trackId: TrackId) => invoke<void>("play_next", { trackId }),
  addToQueue: (trackId: TrackId) => invoke<void>("add_to_queue", { trackId }),
  /** Plusieurs titres dans l'ordre : en tête de la file avec `next` (« Lire ensuite »), sinon à la fin. */
  enqueueTracks: (trackIds: TrackId[], next: boolean) =>
    invoke<void>("enqueue_tracks", { trackIds, next }),
  removeFromQueue: (index: number) => invoke<void>("remove_from_queue", { index }),
  /** Déplace l'entrée `from` de la file manuelle pour qu'elle finisse à l'index `to`. */
  moveInQueue: (from: number, to: number) => invoke<void>("move_in_queue", { from, to }),
  clearQueue: () => invoke<void>("clear_queue"),
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
  uiSettings: () => invoke<UiSettings>("ui_settings"),
  setUiSettings: (settings: UiSettings) => invoke<void>("set_ui_settings", { settings }),
  /** Langue effective (réglage, ou langue du système). */
  language: () => invoke<Lang>("language"),
  /** Enregistre le réglage ; renvoie la langue effective. */
  setLanguage: (language: LanguagePref) => invoke<Lang>("set_language", { language }),
  /** Sections affichées, dans l'ordre de l'écran ; renvoie la disposition enregistrée. */
  setHomeLayout: (sections: { key: string; hidden: boolean }[]) =>
    invoke<HomeLayout>("set_home_layout", { sections }),
  resetHomeLayout: () => invoke<HomeLayout>("reset_home_layout"),
  /** Remplace les épingles ; renvoie la liste enregistrée (validée, sans doublons). */
  setPins: (pins: Pin[]) => invoke<Pin[]>("set_pins", { pins }),
  playbackSettings: () => invoke<PlaybackSettings>("playback_settings"),
  /** Enregistre et applique à chaud (sans recharger le titre en cours). */
  setPlaybackSettings: (settings: PlaybackSettings) =>
    invoke<void>("set_playback_settings", { settings }),
  /** Périphériques de sortie connus de mpv ; la suite arrive par `audio-devices`. */
  audioDevices: () => invoke<AudioDevices>("audio_devices"),
  /** État réel du lancement au démarrage, lu dans le système. */
  autostartEnabled: () => invoke<boolean>("autostart_enabled"),
  /** Renvoie l'état relu après changement. */
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  updateStatus: () => invoke<UpdateInfo | null>("update_status"),
  /** L'installeur ferme l'app puis la relance. */
  installUpdate: () => invoke<void>("install_update"),
  /** Vérifie tout de suite : la mise à jour prête (réémise en `update-ready`), ou `null` si à jour. */
  checkForUpdates: () => invoke<UpdateInfo | null>("check_for_updates"),
};

export const events = {
  onPlayerState: (cb: (s: PlayerState) => void): Promise<UnlistenFn> =>
    listen<PlayerState>("player-state", (e) => cb(e.payload)),
  onQueueState: (cb: (q: QueueView) => void): Promise<UnlistenFn> =>
    listen<QueueView>("queue-state", (e) => cb(e.payload)),
  onUpdateReady: (cb: (u: UpdateInfo) => void): Promise<UnlistenFn> =>
    listen<UpdateInfo>("update-ready", (e) => cb(e.payload)),
  onLikedTracks: (cb: (ids: TrackId[]) => void): Promise<UnlistenFn> =>
    listen<TrackId[]>("liked-tracks", (e) => cb(e.payload)),
  onAudioDevices: (cb: (d: AudioDevices) => void): Promise<UnlistenFn> =>
    listen<AudioDevices>("audio-devices", (e) => cb(e.payload)),
  onToast: (cb: (t: Toast) => void): Promise<UnlistenFn> =>
    listen<Toast>("toast", (e) => cb(e.payload)),
};
