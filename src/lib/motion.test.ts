import { describe, expect, it } from "vitest";
import { motionReduced } from "./motion";

describe("motionReduced", () => {
  it("suit le système par défaut", () => {
    expect(motionReduced("system", true)).toBe(true);
    expect(motionReduced("system", false)).toBe(false);
  });

  it("le réglage l'emporte sur le système", () => {
    expect(motionReduced("reduced", false)).toBe(true);
    expect(motionReduced("full", true)).toBe(false);
  });
});
