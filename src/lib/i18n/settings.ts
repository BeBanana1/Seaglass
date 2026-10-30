// Réglages, thèmes, couleur d'accent, mises à jour.

import { num } from "./state.svelte";

export const fr = {
  title: "Réglages",
  sections: { appearance: "Apparence", playback: "Lecture", app: "Application" },

  language: "Langue",
  languageHint: "Langue de l'interface et des contenus proposés par le service (accueil, mixes).",
  languages: { system: "Système", fr: "Français", en: "English" },

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
  },

  accent: "Couleur d'accent",
  accentHint:
    "Boutons, éléments actifs et titres mis en avant. Par défaut, chaque thème a la sienne " +
    "(accordée à l'album pour Verre de mer, Atelier et Surréalisme).",
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

  startup: "Démarrage",
  autostart: "Lancer Seaglass à l'ouverture de session",
  autostartHint: "Réglage de Windows : il peut aussi être changé dans le Gestionnaire des tâches.",

  updates: "Mises à jour",
  checkUpdates: "Vérifier les mises à jour",
  upToDate: "Seaglass est à jour.",
  updateReady: "Une nouvelle version est prête à être installée.",
  updateAuto: "Vérification automatique au démarrage puis toutes les 6 heures.",
  checking: "Vérification…",
  check: "Vérifier",
  checkFailed: (e: string) => `Vérification des mises à jour impossible : ${e}`,

  about: "À propos",
  disclaimer: "Client non officiel, non affilié à TIDAL.",
  streamingOnly: "Streaming uniquement : aucun flux n'est téléchargé ni enregistré sur disque.",
  license: "Licence GPL-3.0. Lecture assurée par mpv.",

  update: {
    ready: "Mise à jour prête",
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
  },

  accent: "Accent color",
  accentHint:
    "Buttons, active items and highlighted titles. By default, each theme has its own " +
    "(matched to the album for Seaglass, Atelier and Surrealism).",
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

  startup: "Startup",
  autostart: "Launch Seaglass when you sign in",
  autostartHint: "A Windows setting: it can also be changed in Task Manager.",

  updates: "Updates",
  checkUpdates: "Check for updates",
  upToDate: "Seaglass is up to date.",
  updateReady: "A new version is ready to install.",
  updateAuto: "Checked automatically at startup, then every 6 hours.",
  checking: "Checking…",
  check: "Check",
  checkFailed: (e: string) => `Could not check for updates: ${e}`,

  about: "About",
  disclaimer: "Unofficial client, not affiliated with TIDAL.",
  streamingOnly: "Streaming only: no stream is downloaded or saved to disk.",
  license: "Licensed under GPL-3.0. Playback by mpv.",

  update: {
    ready: "Update ready",
    current: (version: string) => `Current version: ${version}`,
    playbackStops: "Playback will stop during the installation (a few seconds).",
    installing: "Installing…",
    install: "Install and restart",
    later: "Later",
    laterHint: "It will install when you close the app",
  },
};
