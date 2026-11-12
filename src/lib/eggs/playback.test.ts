import { describe, expect, it } from "vitest";
import { countLoops, LOOPS_FOR_INFINITY, NO_LOOPS, SEQUENCE, sequenceStep, type Loops } from "./playback";

/** Joue des positions successives (durée 200 s) et renvoie le compte. */
function play(positions: number[], repeatOne = true, id = 7, start: Loops = NO_LOOPS): Loops {
  return positions.reduce((loops, pos) => countLoops(loops, id, pos, 200, repeatOne), start);
}

describe("countLoops", () => {
  it("compte un retour au début après la fin", () => {
    expect(play([0, 100, 198, 0.5]).count).toBe(1);
    expect(play([0, 197, 1, 100, 199, 0.2]).count).toBe(2);
  });

  it("ignore un retour au début en pleine écoute", () => {
    expect(play([0, 100, 0.5]).count).toBe(0);
  });

  it("repart de zéro hors Repeat One ou sur un autre titre", () => {
    const looped = play([0, 199, 0.5]);
    expect(countLoops(looped, 8, 0, 200, true).count).toBe(0);
    expect(countLoops(looped, 7, 0, 200, false).count).toBe(0);
  });

  it("atteint l'infini après dix écoutes", () => {
    const positions = Array.from({ length: LOOPS_FOR_INFINITY }, () => [199, 0.4]).flat();
    expect(play([0, ...positions]).count).toBe(LOOPS_FOR_INFINITY);
  });
});

describe("sequenceStep", () => {
  const run = (keys: string[]) => keys.reduce(sequenceStep, 0);

  it("reconnaît la séquence complète, B et A en majuscules aussi", () => {
    expect(run(SEQUENCE)).toBe(SEQUENCE.length);
    expect(run(SEQUENCE.map((k) => (k.length === 1 ? k.toUpperCase() : k)))).toBe(SEQUENCE.length);
  });

  it("repart sur une touche fausse", () => {
    expect(run(["ArrowUp", "ArrowUp", "x"])).toBe(0);
    expect(run(["ArrowUp", "ArrowDown"])).toBe(0);
  });

  it("un ↑ de trop ne casse pas le début", () => {
    expect(run(["ArrowUp", "ArrowUp", "ArrowUp", ...SEQUENCE.slice(2)])).toBe(SEQUENCE.length);
  });
});
