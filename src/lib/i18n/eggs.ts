// Clins d'œil (#399) : les quelques textes qu'ils affichent.

export const fr = {
  /** 2 février : le jour qui recommence. */
  groundhog: "2 février, encore",
  /** Recherche « 42 ». */
  answer: "La réponse est trouvée. Reste la question.",
  /** Vendredi 13 : la piste 13 des listes devient « 12 bis », comme les étages d'hôtel. */
  thirteen: "bis",
  /** Playlist vide, une fois sur dix. */
  track404: "Piste 404 : introuvable.",
  /** Minuteur réglé sur une heure : ajouté à l'infobulle. */
  sevenYears: " (soit sept ans ailleurs)",
  /** Générique de fin (7 clics sur la version). */
  creditsTitle: "Générique",
  credits: [
    ["Lecture", "mpv · GPL-2.0 ou ultérieure"],
    ["Fenêtre et système", "Tauri · MIT / Apache-2.0"],
    ["Interface", "Svelte, Vite · MIT"],
    ["Moteur", "Rust, tokio, serde, reqwest · MIT / Apache-2.0"],
    ["Mémoire", "SQLite · domaine public"],
    ["Android", "Media3 · Apache-2.0"],
    ["Musique", "Les artistes, sur un service dont Seaglass est un client non officiel"],
    ["Licence de Seaglass", "GPL-3.0"],
  ] as [string, string][],
  creditsEnd: "Merci d'avoir écouté jusqu'au bout.",
  /** 29 février : dernière ligne factice de la file. */
  hiddenTrack: "Piste cachée",
  /** Recherche « kessel » : durée en parsecs. */
  parsecs: (n: number) => `${n.toLocaleString("fr")} pc`,
  /** 21 octobre : infobulle du minuteur en cadran à trois lignes. */
  circuits: (arrival: string, now: string, departure: string) =>
    `Arrivée     ${arrival}
Maintenant  ${now}
Départ      ${departure}`,
  /** Thème Séance : carton de fin de file. */
  theEnd: "Fin",
  /** Thème Ordinateur de bord : refus du lecteur, avant le message d'origine. */
  refusals: {
    unplayable: "Je ne lirai pas ce titre. Ce n'est rien de personnel.",
    stopped: "J'ai arrêté la lecture, pour votre bien. Tout est sous contrôle.",
  },
};

export const en: typeof fr = {
  groundhog: "February 2nd, again",
  answer: "The answer has been found. The question remains.",
  thirteen: "A",
  track404: "Track 404: not found.",
  sevenYears: " (seven years elsewhere)",
  creditsTitle: "Credits",
  credits: [
    ["Playback", "mpv · GPL-2.0 or later"],
    ["Window and system", "Tauri · MIT / Apache-2.0"],
    ["Interface", "Svelte, Vite · MIT"],
    ["Engine", "Rust, tokio, serde, reqwest · MIT / Apache-2.0"],
    ["Memory", "SQLite · public domain"],
    ["Android", "Media3 · Apache-2.0"],
    ["Music", "The artists, on a service Seaglass is an unofficial client of"],
    ["Seaglass license", "GPL-3.0"],
  ],
  creditsEnd: "Thanks for listening to the end.",
  hiddenTrack: "Hidden track",
  parsecs: (n: number) => `${n.toLocaleString("en")} pc`,
  circuits: (arrival: string, now: string, departure: string) =>
    `Arrival    ${arrival}
Present    ${now}
Departed   ${departure}`,
  theEnd: "The End",
  refusals: {
    unplayable: "I will not play this track. It is nothing personal.",
    stopped: "I have stopped playback, for your own good. Everything is under control.",
  },
};
