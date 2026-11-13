// Blind test (#434) dans le backend factice : de quoi parcourir les écrans de la partie.
// La vraie logique (tirage, comparaison des réponses) est en Rust ; ici, une comparaison
// grossière suffit.

import type { BlindAnswer, BlindMode, BlindRoundResult, BlindTestView, PlayerState, Track } from "../ipc";

interface Deps {
  state: PlayerState;
  track: (id: number) => Track | undefined;
  emit: (event: string, payload: unknown) => void;
}

const CLIP = 15;

export function blindTestMock({ state, track, emit }: Deps) {
  let view: BlindTestView | null = null;
  let rounds: Track[] = [];
  let saved: { track: Track | null; timePos: number; duration: number } | null = null;

  const simple = (s: string) =>
    s
      .normalize("NFD")
      .replace(/\p{M}/gu, "")
      .toLowerCase()
      .replace(/\(.*?\)/g, "")
      .replace(/[^\p{L}\p{N}]+/gu, " ")
      .trim();

  function current(): Track | undefined {
    return view ? rounds[view.round - 1] : undefined;
  }

  function choicesFor(t: Track, mode: BlindMode) {
    if (mode !== "choices") return [];
    const others = rounds.filter((o) => o.id !== t.id).slice(0, 3);
    const all = [t, ...others].sort(() => Math.random() - 0.5);
    return all.map((o) => ({ title: o.title, artist: o.artists.map((a) => a.name).join(", ") }));
  }

  function play() {
    const t = current();
    if (!view || !t) return;
    view.clipStart = Math.max(0, Math.floor(t.duration * 0.3));
    view.choices = choicesFor(t, view.mode);
    Object.assign(state, {
      track: null,
      blindTest: true,
      timePos: view.clipStart,
      duration: t.duration,
      paused: false,
      idle: false,
      hasNext: true,
      hasPrevious: true,
    });
    emit("player-state", state);
  }

  function publish() {
    emit("blind-test", view);
  }

  return {
    /** Position avancée par `?ticks=1` : l'extrait s'arrête au bout de 15 s. */
    tick() {
      if (view?.phase === "guess" && !state.paused && state.timePos >= view.clipStart + CLIP) {
        state.paused = true;
      }
    },
    handlers: {
      blind_test_start: ({ trackIds, mode, rounds: count, title }: Record<string, unknown>) => {
        const pool = (trackIds as number[]).map(track).filter((t): t is Track => !!t);
        if (pool.length < 4) throw "il faut au moins 4 titres différents pour un blind test";
        rounds = pool.sort(() => Math.random() - 0.5).slice(0, Number(count));
        saved ??= { track: state.track, timePos: state.timePos, duration: state.duration };
        view = {
          title: String(title),
          mode: mode as BlindMode,
          phase: "guess",
          round: 1,
          rounds: rounds.length,
          score: 0,
          maxScore: rounds.length * 3,
          streak: 0,
          bestStreak: 0,
          clipStart: 0,
          clipLength: CLIP,
          choices: [],
          results: [],
        };
        play();
        publish();
      },
      blind_test_answer: ({ answer }: Record<string, unknown>) => {
        const t = current();
        if (!view || !t || view.phase !== "guess") return;
        const a = answer as BlindAnswer;
        const result: BlindRoundResult = { track: t, title: false, artist: false, points: 0, answer: null, chosen: null, correct: null };
        if (a.kind === "text") {
          const guess = simple(a.text);
          result.answer = a.text;
          result.title = guess.includes(simple(t.title));
          result.artist = t.artists.some((x) => guess.includes(simple(x.name)));
        } else if (a.kind === "choice") {
          const correct = view.choices.findIndex((c) => c.title === t.title);
          result.chosen = a.index;
          result.correct = correct;
          result.title = result.artist = a.index === correct;
        } else if (view.mode === "choices") {
          result.correct = view.choices.findIndex((c) => c.title === t.title);
        }
        result.points = (result.title ? 2 : 0) + (result.artist ? 1 : 0);
        view.score += result.points;
        view.streak = result.points > 0 ? view.streak + 1 : 0;
        view.bestStreak = Math.max(view.bestStreak, view.streak);
        view.results.push(result);
        view.phase = "revealed";
        Object.assign(state, { track: t, paused: false });
        emit("player-state", state);
        publish();
      },
      blind_test_replay: () => {
        if (!view) return;
        Object.assign(state, { timePos: view.clipStart, paused: false });
        emit("player-state", state);
      },
      blind_test_next: () => {
        if (!view) return;
        if (view.phase === "guess") return;
        if (view.round >= view.rounds) {
          view.phase = "finished";
          state.paused = true;
          emit("player-state", state);
        } else {
          view.round += 1;
          view.phase = "guess";
          play();
        }
        publish();
      },
      blind_test_quit: () => {
        view = null;
        Object.assign(state, {
          blindTest: false,
          track: saved?.track ?? null,
          timePos: saved?.timePos ?? 0,
          duration: saved?.duration ?? 0,
          paused: true,
        });
        saved = null;
        emit("player-state", state);
        publish();
      },
      blind_test_state: () => view,
    },
  };
}
