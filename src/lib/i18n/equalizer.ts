// Parametric equalizer (#428): Settings › Playback › Equalizer.

import type { EqFilterKind } from "../ipc";

export const fr = {
  title: "Égaliseur",
  hint: "Corrige la réponse d'un casque ou d'une pièce avec des filtres paramétriques, par exemple un profil AutoEQ. Activé, le signal n'est plus bit-perfect, y compris en mode exclusif.",
  enable: "Activer l'égaliseur",
  enableHint: "Désactivé, le signal reste intact.",
  profile: "Profil",
  noProfile: "Aucun profil pour l'instant : importez un profil AutoEQ ou créez-en un.",
  importAction: "Importer depuis AutoEQ",
  importHint:
    "Collez le contenu d'un fichier « ParametricEQ.txt » d'AutoEQ (autoeq.app, ou le dossier de votre casque dans le dépôt AutoEq), ou choisissez ce fichier.",
  importPlaceholder:
    "Preamp: -6.2 dB\nFilter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70\nFilter 2: ON PK Fc 2386 Hz Gain -2.1 dB Q 1.26\n…",
  chooseFile: "Choisir un fichier…",
  fileTooLarge: "fichier trop volumineux pour un profil AutoEQ",
  name: "Nom du profil",
  namePlaceholder: "Casque, enceintes…",
  confirmImport: "Importer",
  cancel: "Annuler",
  newProfile: "Nouveau profil",
  newProfileName: (n: number) => `Profil ${n}`,
  deleteProfile: "Supprimer ce profil",
  maxProfiles: (n: number) => `${n} profils au plus`,
  preamp: "Préampli (dB)",
  preampAuto: "Automatique",
  preampAutoHint: (db: string) =>
    `Automatique : ${db}, pour compenser le plus fort gain et ne pas écrêter.`,
  filters: "Filtres",
  kinds: {
    peaking: "Pic",
    lowShelf: "Plateau grave",
    highShelf: "Plateau aigu",
  } satisfies Record<EqFilterKind, string>,
  kind: "Type",
  freq: "Fréquence (Hz)",
  gain: "Gain (dB)",
  q: "Q",
  addFilter: "Ajouter un filtre",
  removeFilter: "Retirer ce filtre",
  noFilter: "Aucun filtre : le profil ne change rien.",
  curve: "Réponse de l'égaliseur, de 20 Hz à 20 kHz",
};

export const en: typeof fr = {
  title: "Equalizer",
  hint: "Corrects the response of headphones or a room with parametric filters, such as an AutoEQ profile. When on, the signal is no longer bit-perfect, exclusive mode included.",
  enable: "Enable the equalizer",
  enableHint: "When off, the signal is left untouched.",
  profile: "Profile",
  noProfile: "No profile yet: import an AutoEQ profile or create one.",
  importAction: "Import from AutoEQ",
  importHint:
    "Paste the contents of an AutoEQ “ParametricEQ.txt” file (autoeq.app, or your headphones' folder in the AutoEq repository), or choose that file.",
  importPlaceholder:
    "Preamp: -6.2 dB\nFilter 1: ON LSC Fc 105 Hz Gain 5.5 dB Q 0.70\nFilter 2: ON PK Fc 2386 Hz Gain -2.1 dB Q 1.26\n…",
  chooseFile: "Choose a file…",
  fileTooLarge: "file too large for an AutoEQ profile",
  name: "Profile name",
  namePlaceholder: "Headphones, speakers…",
  confirmImport: "Import",
  cancel: "Cancel",
  newProfile: "New profile",
  newProfileName: (n: number) => `Profile ${n}`,
  deleteProfile: "Delete this profile",
  maxProfiles: (n: number) => `${n} profiles at most`,
  preamp: "Preamp (dB)",
  preampAuto: "Automatic",
  preampAutoHint: (db: string) =>
    `Automatic: ${db}, to offset the largest boost and avoid clipping.`,
  filters: "Filters",
  kinds: {
    peaking: "Peak",
    lowShelf: "Low shelf",
    highShelf: "High shelf",
  },
  kind: "Type",
  freq: "Frequency (Hz)",
  gain: "Gain (dB)",
  q: "Q",
  addFilter: "Add a filter",
  removeFilter: "Remove this filter",
  noFilter: "No filter: the profile changes nothing.",
  curve: "Equalizer response, from 20 Hz to 20 kHz",
};
