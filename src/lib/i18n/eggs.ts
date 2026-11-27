// Easter eggs (#399): the few strings they show.

export const fr = {
  /** February 2: the day that starts over. */
  groundhog: "2 février, encore",
  /** Search for "42". */
  answer: "La réponse est trouvée. Reste la question.",
  /** Friday the 13th: track 13 of the lists becomes "12A", like hotel floors. */
  thirteen: "bis",
  /** Empty playlist, one time in ten. */
  track404: "Piste 404 : introuvable.",
  /** Sleep timer set to one hour: added to the tooltip. */
  sevenYears: " (soit sept ans ailleurs)",
  /** End credits (7 clicks on the version). */
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
  /** February 29: fake last row of the queue. */
  hiddenTrack: "Piste cachée",
  /** Search for "kessel": duration in parsecs. */
  parsecs: (n: number) => `${n.toLocaleString("fr")} pc`,
  /** October 21: the sleep timer's tooltip as a three-line dial. */
  circuits: (arrival: string, now: string, departure: string) =>
    `Arrivée     ${arrival}
Maintenant  ${now}
Départ      ${departure}`,
  /** Séance theme: end-of-queue title card. */
  theEnd: "Fin",
  /** On-board computer theme: the player's refusal, before the original message. */
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
