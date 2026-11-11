// Réglages, thèmes, couleur d'accent, mises à jour.

import { num } from "./state.svelte";

export const fr = {
  title: "Réglages",
  sections: { appearance: "Apparence", playback: "Lecture", app: "Application" },

  language: "Langue",
  languageHint: "Langue de l'interface et des contenus proposés par le service (accueil, mixes).",
  languages: { system: "Système", fr: "Français", en: "English" },

  zoom: "Taille de l'interface",
  zoomHint: "Agrandit ou réduit tout l'affichage. Raccourcis : Ctrl + « + », Ctrl + « - », Ctrl + 0.",
  motion: "Animations",
  motionHint:
    "Fonds animés, fondus et transitions. « Système » suit le réglage « réduire les " +
    "animations » de l'appareil.",
  motions: { system: "Système", reduced: "Réduites", full: "Complètes" },
  theme: "Thème",
  themeHint: "Le thème s'applique tout de suite et reste enregistré.",
  themes: {
    seaglass: {
      name: "Verre de mer",
      description: "Verre dépoli sur la pochette en cours ; les couleurs suivent l'album.",
    },
    studio: { name: "Studio", description: "Console sombre et dense, la chaîne audio au premier plan." },
    atelier: { name: "Atelier", description: "Clair et éditorial, grande typographie, beaucoup d'air." },
    acidwave: {
      name: "Acidwave",
      description: "Néons acides, chrome liquide et hologrammes ; ça pulse pendant la lecture.",
    },
    surreal: {
      name: "Surréalisme",
      description: "Paysage vectoriel onirique teinté par l'album, objets qui flottent.",
    },
    solarpunk: {
      name: "Solarpunk",
      description: "Futur végétal et lumineux : feuillages, or solaire, formes Art nouveau.",
    },
    cyberpunk: {
      name: "Cyberpunk",
      description: "Interface de visée dans une ville néon : jaune signal, cyan, angles coupés.",
    },
    neumorph: {
      name: "Neumorphism",
      description: "Reliefs doux ton sur ton : commandes en relief, rainures en creux, listes à plat.",
    },
    "neumorph-dark": {
      name: "Neumorphism sombre",
      description: "Les mêmes reliefs sur graphite, pour le soir.",
    },
    terminal: {
      name: "Terminal",
      description: "Écran cathodique monochrome, tout en mono ; la couleur d'accent devient celle du phosphore.",
    },
    riso: {
      name: "Risographie",
      description: "Fanzine imprimé en deux encres : papier grainé, trame, léger décalage de repérage.",
    },
    bauhaus: {
      name: "Bauhaus",
      description: "Affiche géométrique : grille stricte, rouge, jaune, bleu, cercles et carrés.",
    },
    aero: {
      name: "Aéro",
      description: "Fiche technique de livrée d'avion : bleu glacier, encre ardoise, jaune signal, repères.",
    },
    alloy: {
      name: "Alliage",
      description: "Carrosserie néo-rétro : aluminium satiné, lignes cassées, feux en pixels ambre.",
    },
    "alloy-dark": {
      name: "Alliage sombre",
      description: "La même carrosserie en gunmetal, calandre en grille, pour le soir.",
    },
  },

  accent: "Couleur d'accent",
  accentHint:
    "Boutons, éléments actifs et titres mis en avant. Par défaut, chaque thème a la sienne " +
    "(accordée à l'album pour Verre de mer, Atelier et Surréalisme). Le choix vaut pour le thème actuel : " +
    "chaque thème garde sa couleur.",
  accentTheme: "Celle du thème",
  accentOther: "Autre couleur",
  accentOtherShort: "Autre…",

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
    track: { label: "Par titre", note: "Chaque titre au même volume : idéal en lecture aléatoire." },
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

  startup: "Démarrage",
  autostart: "Lancer Seaglass à l'ouverture de session",
  autostartHint:
    "Seaglass démarre alors réduit dans la zone de notification. Réglage de Windows : il peut aussi être changé dans le Gestionnaire des tâches.",

  window: "Fenêtre",
  closeAction: "Fermer la fenêtre",
  closeActionHint: "Ce que fait le bouton de fermeture. L'icône de la zone de notification affiche la fenêtre et pilote la lecture.",
  closeActions: { quit: "Quitter Seaglass", tray: "Réduire dans la zone de notification" },
  closeActionNotes: {
    quit: "La lecture s'arrête avec l'app.",
    tray: "La musique continue ; « Quitter » dans le menu de l'icône ferme l'app.",
  },

  discord: "Discord",
  discordPresence: "Afficher l'écoute sur Discord",
  discordHint:
    "Vos contacts voient « Écoute Seaglass » avec le titre, l'artiste et la pochette. Rien en pause. L'app Discord doit être ouverte sur ce PC.",

  updates: "Mises à jour",
  checkUpdates: "Vérifier les mises à jour",
  upToDate: "Seaglass est à jour.",
  updateReady: "Une nouvelle version est prête à être installée.",
  updateAvailable: "Une nouvelle version est disponible.",
  updateDownloading: (v: string) => `Version ${v} disponible, téléchargement en cours…`,
  updateAuto: "Vérification automatique au démarrage puis toutes les 6 heures.",
  updatesManual:
    "Mises à jour automatiques réservées à l'AppImage : télécharger les nouvelles versions sur la page de Seaglass.",
  checking: "Vérification…",
  check: "Vérifier",
  checkFailed: (e: string) => `Vérification des mises à jour impossible : ${e}`,

  tidalLinks: "Liens TIDAL",
  tidalLinksLabel: "Ouvrir les liens TIDAL dans Seaglass",
  tidalLinksOn: "Activé : un lien TIDAL touché (WhatsApp, navigateur…) ouvre sa page dans l'app.",
  tidalLinksOff:
    "Désactivé : les liens TIDAL s'ouvrent dans le navigateur. Dans le réglage, « Ajouter des liens », puis cochez tidal.com.",
  tidalLinksEnable: "Activer",
  tidalLinksChange: "Modifier",

  playHistory: "Historique d'écoute",
  playHistoryHint:
    "Gardé sur cet appareil seulement, pour « Écoutés dans Seaglass » et « À la une ». Rien n'est envoyé à TIDAL.",
  clearPlayHistory: "Effacer",
  playHistoryCleared: "Historique d'écoute effacé",

  changelog: "Journal des versions",
  changelogHint: "Les changements de chaque version qui concernent cet appareil.",
  showChangelog: "Afficher",
  hideChangelog: "Masquer",
  publishedOn: (date: string) => `publiée le ${date}`,
  logs: "Journaux",
  logsHint:
    "Gardés sur cet appareil seulement. Pour signaler un bug, copiez le journal ou le rapport de plantage et collez-le dans l'issue.",
  copyLog: "Copier le journal",
  copyCrash: "Copier le dernier rapport de plantage",
  openLogDir: "Ouvrir le dossier",
  noCrash: "Aucun plantage enregistré",
  logCopied: "Journal copié",
  crashCopied: "Rapport de plantage copié",
  logUnavailable: "Journal indisponible",
  crashed: "Seaglass s'est fermé de façon inattendue — copier le rapport",

  about: "À propos",
  account: "Compte",
  disclaimer: "Client non officiel, non affilié à TIDAL.",
  streamingOnly: "Streaming uniquement : aucun flux n'est téléchargé ni enregistré sur disque.",
  license: "Licence GPL-3.0. Lecture assurée par mpv.",
  sourceCode: "Code source",
  reportIssue: "Signaler un bug ou proposer une idée",
  reportIssueHint: "Un compte sur git.bebanana.ch est nécessaire pour ouvrir une issue.",

  update: {
    ready: "Mise à jour prête",
    available: "Mise à jour disponible",
    download: (mb: string | null) => (mb ? `Télécharger et installer (${mb} Mo)` : "Télécharger et installer"),
    downloading: "Téléchargement…",
    current: (version: string) => `Version actuelle : ${version}`,
    playbackStops: "La lecture s'arrêtera le temps de l'installation (quelques secondes).",
    installing: "Installation…",
    install: "Installer et redémarrer",
    later: "Plus tard",
    laterHint: "Elle s'installera à la fermeture de l'app",
  },
};

export const en: typeof fr = {
  title: "Settings",
  sections: { appearance: "Appearance", playback: "Playback", app: "Application" },

  language: "Language",
  languageHint: "Language of the interface and of the content suggested by the service (home, mixes).",
  languages: { system: "System", fr: "Français", en: "English" },

  zoom: "Interface size",
  zoomHint: "Enlarges or shrinks the whole display. Shortcuts: Ctrl + \"+\", Ctrl + \"-\", Ctrl + 0.",
  motion: "Animations",
  motionHint:
    "Animated backdrops, fades and transitions. \"System\" follows the device's " +
    "\"reduce motion\" setting.",
  motions: { system: "System", reduced: "Reduced", full: "Full" },
  theme: "Theme",
  themeHint: "The theme applies right away and is saved.",
  themes: {
    seaglass: {
      name: "Seaglass",
      description: "Frosted glass over the current cover; colors follow the album.",
    },
    studio: { name: "Studio", description: "Dark, dense console with the signal path up front." },
    atelier: { name: "Atelier", description: "Light and editorial, large type, plenty of air." },
    acidwave: {
      name: "Acidwave",
      description: "Acid neons, liquid chrome and holograms; it pulses while playing.",
    },
    surreal: {
      name: "Surrealism",
      description: "Dreamlike vector landscape tinted by the album, with floating objects.",
    },
    solarpunk: {
      name: "Solarpunk",
      description: "A green, sunlit future: foliage, solar gold, Art Nouveau curves.",
    },
    cyberpunk: {
      name: "Cyberpunk",
      description: "A HUD in a neon city: signal yellow, cyan, cut corners.",
    },
    neumorph: {
      name: "Neumorphism",
      description: "Soft, tone-on-tone relief: raised controls, inset grooves, flat lists.",
    },
    "neumorph-dark": {
      name: "Neumorphism dark",
      description: "The same relief on graphite, for the evening.",
    },
    terminal: {
      name: "Terminal",
      description: "Monochrome CRT, all monospace; the accent color becomes the phosphor.",
    },
    riso: {
      name: "Risograph",
      description: "A two-ink printed zine: grainy paper, halftone, slight misregistration.",
    },
    bauhaus: {
      name: "Bauhaus",
      description: "A geometric poster: strict grid, red, yellow, blue, circles and squares.",
    },
    aero: {
      name: "Aero",
      description: "An aircraft livery spec sheet: ice blue, slate ink, signal yellow, registration marks.",
    },
    alloy: {
      name: "Alloy",
      description: "Neo-retro bodywork: satin aluminium, folded lines, amber pixel lights.",
    },
    "alloy-dark": {
      name: "Alloy dark",
      description: "The same bodywork in gunmetal, with a mesh grille, for the evening.",
    },
  },

  accent: "Accent color",
  accentHint:
    "Buttons, active items and highlighted titles. By default, each theme has its own " +
    "(matched to the album for Seaglass, Atelier and Surrealism). The choice applies to the current theme: " +
    "each theme keeps its own color.",
  accentTheme: "Theme default",
  accentOther: "Other color",
  accentOtherShort: "Other…",

  cache: "Playback cache",
  cacheHint:
    "Memory reserved for reading ahead in each loaded track (current and next). " +
    "64 MB covers about 5 minutes of 24-bit / 48 kHz FLAC, about ten at 16-bit / 44.1 kHz. " +
    "A larger cache copes better with an unstable connection.",
  cacheSize: "Cache size",
  megabytes: (n: number) => `${num(n)} MB`,
  cacheNote: "Applied right away, without interrupting playback. The cache stays in memory: nothing is written to disk.",

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

  startup: "Startup",
  autostart: "Launch Seaglass when you sign in",
  autostartHint:
    "Seaglass then starts minimized to the notification area. A Windows setting: it can also be changed in Task Manager.",

  window: "Window",
  closeAction: "Close the window",
  closeActionHint: "What the close button does. The notification area icon shows the window and controls playback.",
  closeActions: { quit: "Quit Seaglass", tray: "Minimize to the notification area" },
  closeActionNotes: {
    quit: "Playback stops with the app.",
    tray: "The music keeps playing; “Quit” in the icon's menu closes the app.",
  },

  discord: "Discord",
  discordPresence: "Show what you're playing on Discord",
  discordHint:
    "Your friends see “Listening to Seaglass” with the title, artist and cover. Nothing while paused. The Discord app must be running on this PC.",

  updates: "Updates",
  checkUpdates: "Check for updates",
  upToDate: "Seaglass is up to date.",
  updateReady: "A new version is ready to install.",
  updateAvailable: "A new version is available.",
  updateDownloading: (v: string) => `Version ${v} available, downloading…`,
  updateAuto: "Checked automatically at startup, then every 6 hours.",
  updatesManual:
    "Automatic updates are only available in the AppImage: download new versions from the Seaglass page.",
  checking: "Checking…",
  check: "Check",
  checkFailed: (e: string) => `Could not check for updates: ${e}`,

  tidalLinks: "TIDAL links",
  tidalLinksLabel: "Open TIDAL links in Seaglass",
  tidalLinksOn: "On: tapping a TIDAL link (WhatsApp, browser…) opens its page in the app.",
  tidalLinksOff:
    "Off: TIDAL links open in the browser. In the settings page, choose “Add links”, then tick tidal.com.",
  tidalLinksEnable: "Turn on",
  tidalLinksChange: "Change",

  playHistory: "Listening history",
  playHistoryHint:
    "Kept on this device only, for “Played in Seaglass” and “Featured”. Nothing is sent to TIDAL.",
  clearPlayHistory: "Clear",
  playHistoryCleared: "Listening history cleared",

  changelog: "Changelog",
  changelogHint: "What changed in each version, for this device.",
  showChangelog: "Show",
  hideChangelog: "Hide",
  publishedOn: (date: string) => `released ${date}`,
  logs: "Logs",
  logsHint:
    "Kept on this device only. To report a bug, copy the log or the crash report and paste it into the issue.",
  copyLog: "Copy log",
  copyCrash: "Copy last crash report",
  openLogDir: "Open folder",
  noCrash: "No crash recorded",
  logCopied: "Log copied",
  crashCopied: "Crash report copied",
  logUnavailable: "Log unavailable",
  crashed: "Seaglass closed unexpectedly — copy the report",

  about: "About",
  account: "Account",
  disclaimer: "Unofficial client, not affiliated with TIDAL.",
  streamingOnly: "Streaming only: no stream is downloaded or saved to disk.",
  license: "Licensed under GPL-3.0. Playback by mpv.",
  sourceCode: "Source code",
  reportIssue: "Report a bug or suggest an idea",
  reportIssueHint: "Opening an issue requires an account on git.bebanana.ch.",

  update: {
    ready: "Update ready",
    available: "Update available",
    download: (mb: string | null) => (mb ? `Download and install (${mb} MB)` : "Download and install"),
    downloading: "Downloading…",
    current: (version: string) => `Current version: ${version}`,
    playbackStops: "Playback will stop during the installation (a few seconds).",
    installing: "Installing…",
    install: "Install and restart",
    later: "Later",
    laterHint: "It will install when you close the app",
  },
};
