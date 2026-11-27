import { describe, expect, it } from "vitest";
import { capture, displayKeys, sameKeys } from "./hotkeys";

const key = (
  code: string,
  mods: Partial<Record<"ctrlKey" | "altKey" | "shiftKey" | "metaKey", boolean>> = {},
) => ({
  code,
  ctrlKey: false,
  altKey: false,
  shiftKey: false,
  metaKey: false,
  ...mods,
});

describe("capture", () => {
  it("writes the modifiers in order then the key", () => {
    expect(capture(key("KeyL", { shiftKey: true, ctrlKey: true, altKey: true }))).toEqual({
      kind: "keys",
      keys: "Ctrl+Alt+Shift+KeyL",
    });
    expect(capture(key("ArrowUp", { metaKey: true }))).toEqual({
      kind: "keys",
      keys: "Super+ArrowUp",
    });
  });

  it("waits for the key while only modifiers are pressed", () => {
    expect(capture(key("ControlLeft", { ctrlKey: true })).kind).toBe("pending");
    expect(capture(key("AltRight", { altKey: true })).kind).toBe("pending");
  });

  it("exige Ctrl, Alt ou Windows", () => {
    expect(capture(key("KeyA")).kind).toBe("needsModifier");
    expect(capture(key("KeyA", { shiftKey: true })).kind).toBe("needsModifier");
  });

  it("refuses a key the system cannot register", () => {
    expect(capture(key("IntlBackslash", { ctrlKey: true })).kind).toBe("unsupported");
    expect(capture(key("", { ctrlKey: true })).kind).toBe("unsupported");
  });
});

describe("sameKeys", () => {
  it("ignores case and the order of modifiers", () => {
    expect(sameKeys("Ctrl+Alt+KeyP", "alt+ctrl+keyp")).toBe(true);
    expect(sameKeys("Ctrl+Alt+KeyP", "Ctrl+KeyP")).toBe(false);
  });
});

describe("displayKeys", () => {
  it("rend la combinaison lisible", () => {
    expect(displayKeys("Ctrl+Alt+KeyL")).toBe("Ctrl + Alt + L");
    expect(displayKeys("Ctrl+Alt+ArrowUp")).toBe("Ctrl + Alt + ↑");
    expect(displayKeys("Super+Digit5")).toBe("Win + 5");
    expect(displayKeys("Ctrl+Alt+Space", "Espace")).toBe("Ctrl + Alt + Espace");
  });
});
