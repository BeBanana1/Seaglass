// Playback handoff with the computer (#414), phone side: Settings › App and "Now playing".

export const fr = {
  title: "Ordinateur lié",
  hint: "Reprenez sur le téléphone ce qui joue sur l'ordinateur, ou l'inverse, avec Seaglass ouvert sur l'ordinateur et « Accepter les transferts » activé (Réglages › Application › Télécommande). Liez-les sur le même Wi-Fi, après « Lier un téléphone » sur l'ordinateur ; ensuite, ça marche aussi hors de chez vous.",
  online: "Joignable aussi hors de chez vous, par le relais (chiffré de bout en bout).",
  lanOnly:
    "Sur le même réseau seulement : pour le joindre aussi hors de chez vous, appuyez sur « Lier un téléphone » sur l'ordinateur, puis liez-le de nouveau ici.",
  linkLabel: "Adresse de l'ordinateur",
  linkPlaceholder: "192.168.1.20:47801 XXXX-XXXX-XXXX",
  linkHint:
    "Tapez l'adresse et le code affichés sur l'ordinateur (Réglages › Application › Télécommande), séparés par un espace, ou collez l'adresse copiée là-bas.",
  link: "Lier",
  linked: "Ordinateur lié",
  linkedTo: (host: string) => `Lié à ${host}`,
  unlink: "Délier",
  unlinked: "Ordinateur délié",
  pull: "Reprendre ici",
  pullHint: "La lecture de l'ordinateur passe sur ce téléphone ; l'ordinateur se met en pause.",
  push: "Continuer sur l'ordinateur",
  pushHint: "Ce téléphone se met en pause ; l'ordinateur reprend au même endroit.",
  pulled: "Lecture reprise ici",
  pushed: "Lecture transférée sur l'ordinateur",
  menu: "Transférer la lecture",
  history: "Historique d'écoute",
  historyHint:
    "Les écoutes de chaque appareil passent sur l'autre (statistiques, rétrospective, « Écoutés dans Seaglass »), au lancement, à l'ouverture des statistiques et après un transfert. Effacer l'historique n'efface que celui de cet appareil.",
  sync: "Synchroniser",
  synced: "Historique synchronisé",
  syncedAt: (when: string) => `Synchronisé le ${when}`,
  neverSynced: "Jamais synchronisé",
};

export const en: typeof fr = {
  title: "Linked computer",
  hint: "Resume on the phone what plays on the computer, or the other way round, with Seaglass open on the computer and “Accept transfers” turned on (Settings › App › Remote). Link them on the same Wi-Fi, after “Link a phone” on the computer; after that, it also works away from home.",
  online: "Also reachable away from home, through the relay (end-to-end encrypted).",
  lanOnly:
    "Same network only: to also reach it away from home, press “Link a phone” on the computer, then link it again here.",
  linkLabel: "Computer address",
  linkPlaceholder: "192.168.1.20:47801 XXXX-XXXX-XXXX",
  linkHint:
    "Type the address and code shown on the computer (Settings › App › Remote), separated by a space, or paste the address copied there.",
  link: "Link",
  linked: "Computer linked",
  linkedTo: (host: string) => `Linked to ${host}`,
  unlink: "Unlink",
  unlinked: "Computer unlinked",
  pull: "Resume here",
  pullHint: "The computer's playback moves to this phone; the computer pauses.",
  push: "Continue on the computer",
  pushHint: "This phone pauses; the computer resumes at the same spot.",
  pulled: "Playback resumed here",
  pushed: "Playback moved to the computer",
  menu: "Transfer playback",
  history: "Listening history",
  historyHint:
    "Each device's plays show up on the other (stats, recap, “Played in Seaglass”), at launch, when opening the stats and after a transfer. Clearing the history only clears this device's.",
  sync: "Sync",
  synced: "History synced",
  syncedAt: (when: string) => `Synced on ${when}`,
  neverSynced: "Never synced",
};
