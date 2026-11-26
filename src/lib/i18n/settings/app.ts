// Settings › Application: startup, window, integrations, updates, logs, backup, about.

import { num } from "../state.svelte";

export const fr = {
  startup: "Démarrage",
  autostart: "Lancer Seaglass à l'ouverture de session",
  autostartHint:
    "Seaglass démarre alors réduit dans la zone de notification. Réglage de Windows : il peut aussi être changé dans le Gestionnaire des tâches.",

  window: "Fenêtre",
  closeAction: "Fermer la fenêtre",
  closeActionHint:
    "Ce que fait le bouton de fermeture. L'icône de la zone de notification affiche la fenêtre et pilote la lecture.",
  closeActions: { quit: "Quitter Seaglass", tray: "Réduire dans la zone de notification" },
  closeActionNotes: {
    quit: "La lecture s'arrête avec l'app.",
    tray: "La musique continue ; « Quitter » dans le menu de l'icône ferme l'app.",
  },

  discord: "Discord",
  discordPresence: "Afficher l'écoute sur Discord",
  notifications: "Notifications",
  releaseNotifications: "Nouvelles sorties",
  releaseNotificationsHint:
    "Une notification quand un artiste suivi sort un album, un EP ou un single. Vérifié au lancement puis toutes les 6 heures, tant que l'app tourne.",
  releaseNotificationsHintMobile:
    "Une notification quand un artiste suivi sort un album, un EP ou un single. Vérifié toutes les 6 heures tant que l'app tourne ; le canal « Nouvelles sorties » se règle dans les paramètres d'Android.",
  discordHint:
    "Vos contacts voient « Écoute Seaglass » avec le titre, l'artiste et la pochette. Rien en pause. L'app Discord doit être ouverte sur ce PC.",

  listenbrainz: {
    title: "ListenBrainz",
    hint: "Envoie vos écoutes à ListenBrainz, service ouvert d'historique d'écoute. Un titre compte après la moitié de sa durée ou 4 minutes. Désactivé tant qu'aucun jeton n'est collé.",
    token: "Jeton utilisateur",
    tokenHint:
      "À copier depuis listenbrainz.org, Paramètres › Jeton utilisateur. Il reste sur cet appareil.",
    connect: "Vérifier et connecter",
    connecting: "Vérification…",
    connected: (user: string) => `Connecté : ${user}`,
    pending: (n: number) => `${num(n)} écoute${n > 1 ? "s" : ""} en attente d'envoi`,
    refused: "ListenBrainz refuse ce jeton : collez-en un nouveau pour reprendre l'envoi.",
    disconnect: "Retirer",
    disconnectHint: "Les écoutes pas encore envoyées sont abandonnées.",
    done: "ListenBrainz connecté",
  },

  globalShortcuts: "Raccourcis globaux",
  globalShortcutsEnable: "Piloter Seaglass depuis n'importe quelle app",
  globalShortcutsHint:
    "Ces combinaisons marchent même quand la fenêtre n'a pas le focus. Les touches média du clavier n'en ont pas besoin.",
  globalActions: {
    playPause: "Lecture / pause",
    next: "Titre suivant",
    previous: "Titre précédent",
    volumeUp: "Monter le volume",
    volumeDown: "Baisser le volume",
    like: "Aimer le titre en cours",
    showWindow: "Afficher la fenêtre",
  },
  shortcutNone: "Aucun",
  shortcutRecord: (action: string) => `Choisir le raccourci : ${action}`,
  shortcutRecording: "Appuyez sur une combinaison…",
  shortcutClear: (action: string) => `Retirer le raccourci : ${action}`,
  shortcutTaken: "Déjà utilisé par une autre application.",
  shortcutDuplicate: (action: string) => `Déjà utilisé pour « ${action} ».`,
  shortcutNeedsModifier: "Ajoutez Ctrl, Alt ou Windows.",
  shortcutUnsupported: "Cette touche ne peut pas servir de raccourci global.",
  shortcutRecordHint: "Échap pour annuler, Retour arrière pour retirer.",
  shortcutsReset: "Combinaisons par défaut",

  updates: "Mises à jour",
  checkUpdates: "Vérifier les mises à jour",
  upToDate: "Seaglass est à jour.",
  updateReady: "Une nouvelle version est prête à être installée.",
  updateAvailable: "Une nouvelle version est disponible.",
  updateDownloading: (v: string) => `Version ${v} disponible, téléchargement en cours…`,
  updateAuto: "Vérification automatique au démarrage puis toutes les 6 heures.",
  updateManualOnly: "Vérification automatique désactivée : vérifiez avec le bouton.",
  autoUpdates: "Mises à jour automatiques",
  autoUpdatesHint: (server: string) => `Au démarrage puis toutes les 6 heures, sur ${server}.`,
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
    "Gardé dans Seaglass, pour « Écoutés dans Seaglass », « À la une » et les statistiques ; échangé seulement avec l'appareil lié (ordinateur ↔ téléphone). Cet historique n'est pas envoyé à TIDAL, qui reçoit chaque écoute à part, comme avec ses applications. Effacer n'efface que cet appareil.",
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

  libraryExport: "Exporter la bibliothèque",
  libraryExportHint:
    "Titres likés, albums, artistes et playlists (avec leurs titres), en JSON et en CSV dans Documents\\Seaglass. Métadonnées seulement : aucun fichier audio.",
  exportLibrary: "Exporter",
  exportProgress: (done: number, total: number) => `Playlists : ${done} / ${total}`,
  exportDone: (playlists: number, tracks: number) =>
    `Bibliothèque exportée : ${tracks} titre${tracks > 1 ? "s" : ""} likés, ${playlists} playlist${playlists > 1 ? "s" : ""}`,
  exportFailed: (titles: string[]) =>
    `Playlists illisibles, laissées de côté : ${titles.join(", ")}`,
  exportTruncated: "Une liste dépasse 10 000 éléments : seuls les plus récents sont exportés.",

  backup: "Sauvegarde des réglages",
  backupHint:
    "Thème, couleurs, épingles, accueil, tris et réglages de lecture, en texte à coller dans Seaglass sur un autre appareil. Ni compte, ni file, ni historique.",
  copySettings: "Copier mes réglages",
  settingsCopied: "Réglages copiés",
  importSettings: "Importer…",
  importPlaceholder: "Collez ici des réglages copiés depuis Seaglass",
  applyImport: "Appliquer",
  settingsImported: (applied: number, ignored: string[]) =>
    `${applied} réglage${applied > 1 ? "s" : ""} repris` +
    (ignored.length ? ` ; ignorés : ${ignored.join(", ")}` : ""),

  about: "À propos",
  account: "Compte",
  disclaimer: "Client non officiel, non affilié à TIDAL.",
  clientNotice:
    "Seaglass se connecte avec l'identifiant de l'app Android de TIDAL, que TIDAL n'autorise pas pour d'autres apps : l'utiliser peut aller contre les conditions d'utilisation de TIDAL.",
  clientNoticeMore: "En savoir plus",
  streamingOnly: "Streaming uniquement : aucun flux n'est téléchargé ni enregistré sur disque.",
  license: "Licence GPL-3.0. Lecture assurée par mpv.",
  website: "Site web",
  sourceCode: "Code source",
  reportIssue: "Signaler un bug ou proposer une idée",
  reportIssueHint: "Un compte GitHub est nécessaire pour ouvrir une issue.",

  update: {
    ready: "Mise à jour prête",
    available: "Mise à jour disponible",
    download: (mb: string | null) =>
      mb ? `Télécharger et installer (${mb} Mo)` : "Télécharger et installer",
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
  startup: "Startup",
  autostart: "Launch Seaglass when you sign in",
  autostartHint:
    "Seaglass then starts minimized to the notification area. A Windows setting: it can also be changed in Task Manager.",

  window: "Window",
  closeAction: "Close the window",
  closeActionHint:
    "What the close button does. The notification area icon shows the window and controls playback.",
  closeActions: { quit: "Quit Seaglass", tray: "Minimize to the notification area" },
  closeActionNotes: {
    quit: "Playback stops with the app.",
    tray: "The music keeps playing; “Quit” in the icon's menu closes the app.",
  },

  discord: "Discord",
  discordPresence: "Show what you're playing on Discord",
  notifications: "Notifications",
  releaseNotifications: "New releases",
  releaseNotificationsHint:
    "A notification when a followed artist releases an album, EP or single. Checked at startup, then every 6 hours, while the app is running.",
  releaseNotificationsHintMobile:
    "A notification when a followed artist releases an album, EP or single. Checked every 6 hours while the app is running; the “New releases” channel can be set in Android settings.",
  discordHint:
    "Your friends see “Listening to Seaglass” with the title, artist and cover. Nothing while paused. The Discord app must be running on this PC.",

  listenbrainz: {
    title: "ListenBrainz",
    hint: "Sends your listens to ListenBrainz, an open listening history service. A track counts after half its length or 4 minutes. Off until a token is pasted.",
    token: "User token",
    tokenHint: "Copy it from listenbrainz.org, Settings › User token. It stays on this device.",
    connect: "Check and connect",
    connecting: "Checking…",
    connected: (user: string) => `Connected: ${user}`,
    pending: (n: number) => `${num(n)} listen${n > 1 ? "s" : ""} waiting to be sent`,
    refused: "ListenBrainz refuses this token: paste a new one to resume sending.",
    disconnect: "Remove",
    disconnectHint: "Listens not sent yet are dropped.",
    done: "ListenBrainz connected",
  },

  globalShortcuts: "Global shortcuts",
  globalShortcutsEnable: "Control Seaglass from any app",
  globalShortcutsHint:
    "These combinations work even when the window isn't focused. The keyboard's media keys don't need them.",
  globalActions: {
    playPause: "Play / pause",
    next: "Next track",
    previous: "Previous track",
    volumeUp: "Volume up",
    volumeDown: "Volume down",
    like: "Like the current track",
    showWindow: "Show the window",
  },
  shortcutNone: "None",
  shortcutRecord: (action: string) => `Choose the shortcut: ${action}`,
  shortcutRecording: "Press a combination…",
  shortcutClear: (action: string) => `Remove the shortcut: ${action}`,
  shortcutTaken: "Already used by another application.",
  shortcutDuplicate: (action: string) => `Already used for “${action}”.`,
  shortcutNeedsModifier: "Add Ctrl, Alt or Windows.",
  shortcutUnsupported: "This key can't be used as a global shortcut.",
  shortcutRecordHint: "Esc to cancel, Backspace to remove.",
  shortcutsReset: "Default combinations",

  updates: "Updates",
  checkUpdates: "Check for updates",
  upToDate: "Seaglass is up to date.",
  updateReady: "A new version is ready to install.",
  updateAvailable: "A new version is available.",
  updateDownloading: (v: string) => `Version ${v} available, downloading…`,
  updateAuto: "Checked automatically at startup, then every 6 hours.",
  updateManualOnly: "Automatic checks are off: use the button.",
  autoUpdates: "Automatic updates",
  autoUpdatesHint: (server: string) => `At startup, then every 6 hours, from ${server}.`,
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
    "Kept in Seaglass, for “Played in Seaglass”, “Featured” and the stats; only exchanged with the linked device (computer ↔ phone). This history is not sent to TIDAL, which is told of each listen separately, as with its own apps. Clearing only clears this device.",
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

  libraryExport: "Export library",
  libraryExportHint:
    "Liked tracks, albums, artists and playlists (with their tracks), as JSON and CSV in Documents\\Seaglass. Metadata only: no audio files.",
  exportLibrary: "Export",
  exportProgress: (done: number, total: number) => `Playlists: ${done} / ${total}`,
  exportDone: (playlists: number, tracks: number) =>
    `Library exported: ${tracks} liked track${tracks > 1 ? "s" : ""}, ${playlists} playlist${playlists > 1 ? "s" : ""}`,
  exportFailed: (titles: string[]) => `Unreadable playlists, skipped: ${titles.join(", ")}`,
  exportTruncated: "A list has more than 10,000 items: only the most recent are exported.",

  backup: "Settings backup",
  backupHint:
    "Theme, colours, pins, home, sorting and playback settings, as text to paste into Seaglass on another device. No account, queue or history.",
  copySettings: "Copy my settings",
  settingsCopied: "Settings copied",
  importSettings: "Import…",
  importPlaceholder: "Paste settings copied from Seaglass here",
  applyImport: "Apply",
  settingsImported: (applied: number, ignored: string[]) =>
    `${applied} setting${applied > 1 ? "s" : ""} restored` +
    (ignored.length ? `; ignored: ${ignored.join(", ")}` : ""),

  about: "About",
  account: "Account",
  disclaimer: "Unofficial client, not affiliated with TIDAL.",
  clientNotice:
    "Seaglass signs in with the identifier of TIDAL's Android app, which TIDAL has not authorized for other apps: using it may go against TIDAL's terms of use.",
  clientNoticeMore: "Learn more",
  streamingOnly: "Streaming only: no stream is downloaded or saved to disk.",
  license: "Licensed under GPL-3.0. Playback by mpv.",
  website: "Website",
  sourceCode: "Source code",
  reportIssue: "Report a bug or suggest an idea",
  reportIssueHint: "Opening an issue requires a GitHub account.",

  update: {
    ready: "Update ready",
    available: "Update available",
    download: (mb: string | null) =>
      mb ? `Download and install (${mb} MB)` : "Download and install",
    downloading: "Downloading…",
    current: (version: string) => `Current version: ${version}`,
    playbackStops: "Playback will stop during the installation (a few seconds).",
    installing: "Installing…",
    install: "Install and restart",
    later: "Later",
    laterHint: "It will install when you close the app",
  },
};
