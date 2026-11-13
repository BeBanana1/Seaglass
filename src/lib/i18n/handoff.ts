// Transfert de la lecture avec l'ordinateur (#414), côté téléphone : Réglages › Application
// et « En lecture ».

export const fr = {
  title: "Ordinateur lié",
  hint: "Reprenez sur le téléphone ce qui joue sur l'ordinateur, ou l'inverse. Sur le même Wi-Fi, avec Seaglass ouvert sur l'ordinateur et « Accepter les transferts » activé (Réglages › Application › Télécommande).",
  linkLabel: "Adresse de l'ordinateur",
  linkPlaceholder: "192.168.1.20:47801 XXXX-XXXX-XXXX",
  linkHint: "Tapez l'adresse et le code affichés sur l'ordinateur (Réglages › Application › Télécommande), séparés par un espace, ou collez l'adresse copiée là-bas.",
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
};

export const en: typeof fr = {
  title: "Linked computer",
  hint: "Resume on the phone what plays on the computer, or the other way round. Same Wi-Fi, with Seaglass open on the computer and “Accept transfers” turned on (Settings › App › Remote).",
  linkLabel: "Computer address",
  linkPlaceholder: "192.168.1.20:47801 XXXX-XXXX-XXXX",
  linkHint: "Type the address and code shown on the computer (Settings › App › Remote), separated by a space, or paste the address copied there.",
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
};
