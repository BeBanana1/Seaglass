import { describe, expect, it } from "vitest";
import { motionReduced } from "./motion";

describe("motionReduced", () => {
  it("follows the system by default", () => {
    expect(motionReduced("system", true)).toBe(true);
    expect(motionReduced("system", false)).toBe(false);
  });

  it("the setting overrides the system", () => {
    expect(motionReduced("reduced", false)).toBe(true);
    expect(motionReduced("full", true)).toBe(false);
  });
});
