// Blind test (#434) in the mock backend: enough to go through the game's screens.
// The real logic (draw, answer matching) is in Rust; a rough comparison is enough here.
// With the Jam (#441, `?jam=1`), two mock guests play: Léa answers as soon as the round
// starts and finds the artist, Max never answers.

import type {
  BlindAnswer,
  BlindMode,
  BlindPlayer,
  BlindPlayerResult,
  BlindRoundResult,
  BlindTestView,
  PlayerState,
  Track,
} from "../ipc";

interface Deps {
  state: PlayerState;
  track: (id: number) => Track | undefined;
  emit: (event: string, payload: unknown) => void;
}

const CLIP = 15;
/** Simulated loading time of a clip (ms). */
const LOAD_DELAY = 1500;

export function blindTestMock({ state, track, emit }: Deps) {
  let view: BlindTestView | null = null;
  let rounds: Track[] = [];
  let saved: { track: Track | null; timePos: number; duration: number } | null = null;
  /** The host's answer to the current round, until the reveal. */
  let hostPending: BlindRoundResult | null = null;

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

  function sortPlayers() {
    view?.players.sort((a, b) => b.score - a.score || a.id - b.id);
  }

  function play() {
    const t = current();
    if (!view || !t) return;
    view.clipStart = Math.max(0, Math.floor(t.duration * 0.3));
    view.choices = choicesFor(t, view.mode);
    view.revealAt = view.party.jam ? Date.now() + 35_000 : null;
    hostPending = null;
    for (const p of view.players) p.answered = p.name === "Léa";
    // Simulated loading (#445): the clip only starts after a while.
    view.clipPlaying = false;
    Object.assign(state, {
      track: null,
      blindTest: true,
      timePos: view.clipStart,
      duration: t.duration,
      paused: false,
      loading: true,
      idle: false,
      hasNext: true,
      hasPrevious: true,
    });
    emit("player-state", state);
    const round = view.round;
    setTimeout(() => {
      if (view?.phase !== "guess" || view.round !== round) return;
      view.clipPlaying = true;
      state.loading = false;
      emit("player-state", state);
      publish();
    }, LOAD_DELAY);
  }

  function publish() {
    emit("blind-test", view);
  }

  function judge(t: Track, a: BlindAnswer): BlindRoundResult {
    const result: BlindRoundResult = {
      track: t,
      title: false,
      artist: false,
      points: 0,
      answer: null,
      chosen: null,
      correct: null,
      players: [],
    };
    if (a.kind === "text") {
      const guess = simple(a.text);
      result.answer = a.text;
      result.title = guess.includes(simple(t.title));
      result.artist = t.artists.some((x) => guess.includes(simple(x.name)));
    } else if (a.kind === "choice") {
      const correct = view!.choices.findIndex((c) => c.title === t.title);
      result.chosen = a.index;
      result.title = result.artist = a.index === correct;
    }
    if (view!.mode === "choices")
      result.correct = view!.choices.findIndex((c) => c.title === t.title);
    result.points = (result.title ? 2 : 0) + (result.artist ? 1 : 0);
    return result;
  }

  function reveal() {
    const t = current();
    if (!view || !t || view.phase !== "guess") return;
    const host = hostPending ?? { ...judge(t, { kind: "pass" }), answer: null };
    const hostAnswered = hostPending !== null;
    const results: BlindPlayerResult[] = view.players.map((p) => {
      const lea = p.name === "Léa";
      const r = p.host
        ? { answered: hostAnswered, title: host.title, artist: host.artist, points: host.points }
        : { answered: lea, title: false, artist: lea, points: lea ? 1 : 0 };
      const fastest = p.host && host.title;
      const points = r.points + (fastest ? 1 : 0);
      p.score += points;
      p.streak = points > 0 ? p.streak + 1 : 0;
      p.bestStreak = Math.max(p.bestStreak, p.streak);
      p.answered = false;
      return { id: p.id, name: p.name, host: p.host, ...r, points, fastest };
    });
    results.sort((a, b) => b.points - a.points || a.id - b.id);
    const hostResult = results.find((r) => r.host);
    host.points = hostResult?.points ?? 0;
    host.players = results;
    view.results.push(host);
    const me = view.players.find((p) => p.host);
    view.score = me?.score ?? 0;
    view.streak = me?.streak ?? 0;
    view.bestStreak = me?.bestStreak ?? 0;
    view.phase = "revealed";
    view.revealAt = null;
    sortPlayers();
    Object.assign(state, { track: t, paused: false });
    emit("player-state", state);
    publish();
  }

  return {
    /** Position moved on by `?ticks=1`: the clip stops after 15 s. */
    tick() {
      if (view?.phase === "guess" && !state.paused && state.timePos >= view.clipStart + CLIP) {
        state.paused = true;
      }
    },
    handlers: {
      blind_test_start: ({
        trackIds,
        mode,
        rounds: count,
        title,
        jam,
        hostPlays,
      }: Record<string, unknown>) => {
        const pool = (trackIds as number[]).map(track).filter((t): t is Track => !!t);
        if (pool.length < 4) throw "il faut au moins 4 titres différents pour un blind test";
        rounds = pool.sort(() => Math.random() - 0.5).slice(0, Number(count));
        saved ??= { track: state.track, timePos: state.timePos, duration: state.duration };
        const together = Boolean(jam);
        const hostIn = !together || hostPlays !== false;
        const player = (id: number, name: string | null, host = false): BlindPlayer => ({
          id,
          name,
          host,
          score: 0,
          streak: 0,
          bestStreak: 0,
          answered: false,
        });
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
          party: { jam: together, hostPlays: hostIn },
          players: [
            ...(hostIn ? [player(0, null, true)] : []),
            ...(together ? [player(1, "Léa"), player(2, "Max")] : []),
          ],
          revealAt: null,
          clipPlaying: false,
        };
        play();
        publish();
      },
      blind_test_answer: ({ answer }: Record<string, unknown>) => {
        const t = current();
        if (!view || !t || view.phase !== "guess" || hostPending) return;
        hostPending = judge(t, answer as BlindAnswer);
        const me = view.players.find((p) => p.host);
        if (me) me.answered = true;
        // Alone: revealed at once; with guests, Max never answers, so "Reveal" is needed.
        if (!view.party.jam) return reveal();
        publish();
      },
      blind_test_reveal: () => reveal(),
      blind_test_replay: () => {
        if (!view) return;
        Object.assign(state, { timePos: view.clipStart, paused: false });
        emit("player-state", state);
      },
      blind_test_next: () => {
        if (!view) return;
        if (view.phase === "guess") return view.party.jam ? reveal() : undefined;
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
