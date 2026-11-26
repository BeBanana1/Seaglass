// Settings › Playback: cache, output, quality, normalization, queue end, content, blocks.

import { num } from "../state.svelte";

export const fr = {
  cache: "Cache de lecture",
  cacheHint:
    "Mémoire réservée à la lecture en avance de chaque titre chargé (en cours et suivant). " +
    "64 Mo couvrent environ 5 minutes de FLAC 24 bits / 48 kHz, une dizaine en 16 bits / " +
    "44,1 kHz. Un cache plus grand résiste mieux à une connexion instable.",
  cacheSize: "Taille du cache",
  megabytes: (n: number) => `${num(n)} Mo`,
  cacheNote:
    "Appliqué tout de suite, sans interrompre la lecture. Le cache reste en mémoire : rien " +
    "n'est écrit sur disque.",

  normalization: "Normalisation du volume",
  normalizationHint:
    "Ramène les titres au même volume perçu avec le ReplayGain fourni par le service, sans " +
    "jamais écrêter. Le signal n'est alors plus bit-perfect.",
  normalizationModes: {
    off: { label: "Désactivée", note: "Le signal est transmis tel quel." },
    track: {
      label: "Par titre",
      note: "Chaque titre au même volume : idéal en lecture aléatoire.",
    },
    album: {
      label: "Par album",
      note: "Les écarts voulus entre les titres d'un album sont gardés (gain du titre s'il manque).",
    },
  },
  normalizationExclusive: "Suspendue en mode exclusif : le signal est transmis tel quel.",
  outputDevice: "Sortie audio",
  outputDeviceHint:
    "Périphérique sur lequel Seaglass joue, sans changer celui des autres applications. " +
    "S'il est débranché, la lecture passe sur le périphérique par défaut et y revient au " +
    "rebranchement.",
  pauseOnUnplug: "Mettre en pause si la sortie est débranchée",
  pauseOnUnplugHint:
    "Casque débranché, DAC éteint ou casque Bluetooth déconnecté : la lecture s'arrête au " +
    "lieu de continuer sur les haut-parleurs.",
  deviceProfiles: "Réglages propres à chaque sortie",
  deviceProfilesHint:
    "Volume, normalisation et mode exclusif sont retenus pour chaque périphérique (DAC, " +
    "casque, enceintes) et reviennent quand il redevient la sortie, y compris en sortie par " +
    "défaut quand Windows en change.",
  resumeOnLaunch: "Reprendre la lecture au lancement",
  resumeOnLaunchHint:
    "Seaglass reprend le dernier titre là où il s'était arrêté. Jamais quand il est lancé " +
    "à l'ouverture de session, ni après un plantage.",
  exclusive: "Mode exclusif",
  exclusiveHint:
    "Seaglass ouvre la sortie audio pour lui seul, sans passer par le mixeur de Windows : " +
    "le flux arrive intact si le périphérique accepte son format. Les autres applications " +
    "n'ont plus de son pendant la lecture, et la normalisation est suspendue.",
  systemDefault: "Par défaut du système",
  deviceUnplugged: "débranché",
  devicesLoading: "Recherche des périphériques…",

  quality: "Qualité",
  qualityHint:
    "Un titre qui n'existe qu'en qualité CD a souvent une autre édition en hi-res (album " +
    "d'origine, réédition, compilation). Le master peut différer de l'édition affichée.",
  mobileQualityHint:
    "En Bluetooth, le son est réencodé par le téléphone : la qualité Haute s'entend comme le " +
    "Lossless et demande environ trois fois moins de données.",
  mobileQualities: {
    low: {
      label: "Basse",
      note: "AAC 96 kbit/s, environ dix fois moins de données que le Lossless.",
    },
    high: { label: "Haute", note: "AAC 320 kbit/s. Moins de données mobiles et de batterie." },
    lossless: { label: "Lossless", note: "FLAC 16 bits / 44,1 kHz, environ 1 Mbit/s." },
    max: {
      label: "Max",
      note:
        "FLAC jusqu'à 24 bits / 192 kHz quand le titre existe en hi-res, jusqu'à 5 Mbit/s. " +
        "Android ramène le son à 16 bits ; utile surtout avec un DAC USB.",
    },
  },
  mobileQualityNext: "Pris en compte à partir du titre suivant.",
  networks: { wifiQuality: "En Wi-Fi", cellularQuality: "En données mobiles" },
  lowBatteryQuality: "Baisser la qualité quand la batterie est faible",
  lowBatteryQualityHint:
    "Sous 50 % : Haute au plus. Sous 20 % ou avec l'économiseur de batterie : Basse. " +
    "Rien ne change pendant la charge.",
  /** #492: current battery cap; `quality` and `chosen` already translated. */
  qualityCap: (quality: string, chosen: string, powerSave: boolean, battery: number | null) =>
    `Pour l'instant : ${quality} au lieu de ${chosen}, ` +
    (powerSave
      ? "économiseur de batterie actif."
      : battery === null
        ? "batterie faible."
        : `batterie à ${battery} %.`),
  hiresEdition: "Édition hi-res",
  hiresModes: {
    auto: {
      label: "Automatique",
      note: "L'édition hi-res est lue à la place du titre, sans demander. La chaîne audio l'indique.",
    },
    ask: {
      label: "Demander",
      note:
        "Le titre d'origine démarre, et la barre de lecture propose l'édition hi-res. " +
        "La réponse vaut pour tout l'album.",
    },
    off: { label: "Désactivé", note: "Le titre est toujours lu depuis l'édition affichée." },
  },

  queueEnd: "Fin de la file",
  autoRadio: "Continuer avec une radio",
  autoRadioHint:
    "Quand la file se termine, enchaîner sur la radio du dernier titre, sans blanc. " +
    "Sans effet en mode répétition.",

  content: "Contenu",
  hideExplicit: "Masquer le contenu explicite",
  hideExplicitHint:
    "Les titres et albums marqués « E » disparaissent de la recherche, des pages, de l'accueil, " +
    "des mixes et des radios. Vos favoris et vos playlists restent complets.",
  hideAi: "Masquer la musique générée par IA",
  hideAiHint:
    "Titres et albums que le service marque comme générés par IA (badge « IA »). Même portée : " +
    "vos favoris et vos playlists restent complets. Le marquage vient du service et peut en manquer.",
  // #334: blocks saved at the service.
  blocked: "Titres et artistes bloqués",
  blockedHint:
    "Ils ne sont plus proposés par les mixes ni par les radios, sur tous vos appareils. Ils restent " +
    "lisibles depuis un album ou une playlist. Pour bloquer, passez par le menu « … » d'un titre.",
  showBlocked: "Afficher la liste",
  blockedTracks: "Titres",
  blockedArtists: "Artistes",
  blockedEmpty: "Rien n'est bloqué.",
  unblock: "Débloquer",
};

export const en: typeof fr = {
  cache: "Playback cache",
  cacheHint:
    "Memory reserved for reading ahead in each loaded track (current and next). " +
    "64 MB covers about 5 minutes of 24-bit / 48 kHz FLAC, about ten at 16-bit / 44.1 kHz. " +
    "A larger cache copes better with an unstable connection.",
  cacheSize: "Cache size",
  megabytes: (n: number) => `${num(n)} MB`,
  cacheNote:
    "Applied right away, without interrupting playback. The cache stays in memory: nothing is written to disk.",

  normalization: "Volume normalization",
  normalizationHint:
    "Brings tracks to the same perceived loudness using the ReplayGain provided by the " +
    "service, without ever clipping. The signal is then no longer bit-perfect.",
  normalizationModes: {
    off: { label: "Off", note: "The signal is passed through untouched." },
    track: { label: "Per track", note: "Every track at the same loudness: best for shuffle." },
    album: {
      label: "Per album",
      note: "Keeps the intended differences between an album's tracks (track gain if missing).",
    },
  },
  normalizationExclusive: "Suspended in exclusive mode: the signal is passed through untouched.",
  outputDevice: "Audio output",
  outputDeviceHint:
    "Device Seaglass plays on, without changing the one used by other apps. If it is " +
    "unplugged, playback moves to the default device and comes back when it is plugged in again.",
  pauseOnUnplug: "Pause when the output is unplugged",
  pauseOnUnplugHint:
    "Headphones unplugged, DAC switched off or Bluetooth headset disconnected: playback " +
    "stops instead of carrying on through the speakers.",
  deviceProfiles: "Separate settings for each output",
  deviceProfilesHint:
    "Volume, normalization and exclusive mode are remembered for each device (DAC, " +
    "headphones, speakers) and come back when it is the output again, including on the " +
    "default output when Windows switches it.",
  resumeOnLaunch: "Resume playback on launch",
  resumeOnLaunchHint:
    "Seaglass picks the last track up where it stopped. Never when it is launched at " +
    "sign-in, nor after a crash.",
  exclusive: "Exclusive mode",
  exclusiveHint:
    "Seaglass opens the audio output for itself, bypassing the Windows mixer: the stream " +
    "arrives untouched if the device accepts its format. Other apps are silent during " +
    "playback, and normalization is suspended.",
  systemDefault: "System default",
  deviceUnplugged: "unplugged",
  devicesLoading: "Looking for devices…",

  quality: "Quality",
  qualityHint:
    "A track only available in CD quality often has another edition in hi-res (original " +
    "album, reissue, compilation). The master may differ from the edition shown.",
  mobileQualityHint:
    "Over Bluetooth the phone re-encodes the sound: High sounds like Lossless and needs " +
    "about a third of the data.",
  mobileQualities: {
    low: { label: "Low", note: "AAC 96 kbit/s, about a tenth of the data of Lossless." },
    high: { label: "High", note: "AAC 320 kbit/s. Less mobile data and battery." },
    lossless: { label: "Lossless", note: "FLAC 16-bit / 44.1 kHz, about 1 Mbit/s." },
    max: {
      label: "Max",
      note:
        "FLAC up to 24-bit / 192 kHz when the track exists in hi-res, up to 5 Mbit/s. " +
        "Android brings the sound down to 16 bits; mostly useful with a USB DAC.",
    },
  },
  mobileQualityNext: "Applies from the next track.",
  networks: { wifiQuality: "On Wi-Fi", cellularQuality: "On mobile data" },
  lowBatteryQuality: "Lower the quality when the battery is low",
  lowBatteryQualityHint:
    "Below 50%: High at most. Below 20% or with battery saver: Low. " +
    "Nothing changes while charging.",
  qualityCap: (quality: string, chosen: string, powerSave: boolean, battery: number | null) =>
    `For now: ${quality} instead of ${chosen}, ` +
    (powerSave
      ? "battery saver is on."
      : battery === null
        ? "battery is low."
        : `battery at ${battery}%.`),
  hiresEdition: "Hi-res edition",
  hiresModes: {
    auto: {
      label: "Automatic",
      note: "The hi-res edition plays instead of the track, without asking. The signal path shows it.",
    },
    ask: {
      label: "Ask",
      note: "The original track starts and the player bar offers the hi-res edition. The answer applies to the whole album.",
    },
    off: { label: "Off", note: "The track always plays from the edition shown." },
  },

  queueEnd: "End of queue",
  autoRadio: "Continue with a radio",
  autoRadioHint:
    "When the queue ends, carry on with the last track's radio, without a gap. " +
    "No effect while repeat is on.",

  content: "Content",
  hideExplicit: "Hide explicit content",
  hideExplicitHint:
    "Tracks and albums marked “E” disappear from search, pages, home, mixes and radios. " +
    "Your favourites and your own playlists stay complete.",
  hideAi: "Hide AI-generated music",
  hideAiHint:
    "Tracks and albums the service marks as AI-generated (“AI” badge). Same scope: your " +
    "favourites and your own playlists stay complete. The label comes from the service and may miss some.",
  blocked: "Blocked tracks and artists",
  blockedHint:
    "Mixes and radios no longer suggest them, on all your devices. They can still be played from " +
    "an album or a playlist. To block one, use a track's “…” menu.",
  showBlocked: "Show the list",
  blockedTracks: "Tracks",
  blockedArtists: "Artists",
  blockedEmpty: "Nothing is blocked.",
  unblock: "Unblock",
};
