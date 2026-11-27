// Global shortcuts (#413): capturing a key combination from the keyboard and displaying it.
// Pure logic, tested (`hotkeys.test.ts`). The format (`Ctrl+Alt+KeyL`) is the one read by
// `global-hotkey` on the Rust side; the key is `KeyboardEvent.code`, independent of the layout.

import type { GlobalAction } from "./ipc";

/** Order of the actions in Settings. */
export const GLOBAL_ACTIONS: GlobalAction[] = [
  "playPause",
  "next",
  "previous",
  "volumeUp",
  "volumeDown",
  "like",
  "showWindow",
];

/** Keys that `global-hotkey` can register (without the modifiers). */
const SUPPORTED =
  /^(Key[A-Z]|Digit[0-9]|Numpad([0-9]|Add|Decimal|Divide|Enter|Equal|Multiply|Subtract)|F([1-9]|1[0-9]|2[0-4])|Arrow(Up|Down|Left|Right)|Space|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Pause|PrintScreen|ScrollLock|Backquote|Backslash|BracketLeft|BracketRight|Comma|Equal|Minus|Period|Quote|Semicolon|Slash|AudioVolume(Up|Down|Mute)|Media(PlayPause|Stop|TrackNext|TrackPrevious))$/;

const MODIFIER = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

export type Capture =
  /** Complete combination. */
  | { kind: "keys"; keys: string }
  /** Only modifiers so far: wait for the key. */
  | { kind: "pending" }
  /** No Ctrl, Alt or Windows: the key would be used to type elsewhere. */
  | { kind: "needsModifier" }
  | { kind: "unsupported" };

type KeyInfo = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

export function capture(e: KeyInfo): Capture {
  if (MODIFIER.test(e.code)) return { kind: "pending" };
  if (!SUPPORTED.test(e.code)) return { kind: "unsupported" };
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return { kind: "needsModifier" };
  const parts = [
    e.ctrlKey && "Ctrl",
    e.altKey && "Alt",
    e.shiftKey && "Shift",
    e.metaKey && "Super",
    e.code,
  ];
  return { kind: "keys", keys: parts.filter(Boolean).join("+") };
}

/** Same combination written differently (case, order of modifiers)? */
export function sameKeys(a: string, b: string): boolean {
  const norm = (k: string) => {
    const parts = k.toUpperCase().split("+");
    const key = parts.pop();
    return [...parts.sort(), key].join("+");
  };
  return norm(a) === norm(b);
}

const KEY_LABELS: Record<string, string> = {
  ArrowUp: "↑",
  ArrowDown: "↓",
  ArrowLeft: "←",
  ArrowRight: "→",
  Super: "Win",
  Backquote: "`",
  Backslash: "\\",
  BracketLeft: "[",
  BracketRight: "]",
  Comma: ",",
  Equal: "=",
  Minus: "-",
  Period: ".",
  Quote: "'",
  Semicolon: ";",
  Slash: "/",
};

/** Displayed text: `Ctrl+Alt+KeyL` → `Ctrl + Alt + L`, `Ctrl+Alt+ArrowUp` → `Ctrl + Alt + ↑`.
 *  `space`: name of the space bar in the current language. */
export function displayKeys(keys: string, space = "Space"): string {
  return keys
    .split("+")
    .map((part) => {
      if (part === "Space") return space;
      if (KEY_LABELS[part]) return KEY_LABELS[part];
      return part.replace(/^Key|^Digit/, "").replace(/^Numpad/, "Num ");
    })
    .join(" + ");
}

/** Proposed combinations, the same as `GlobalShortcuts::default` on the Rust side. */
export const DEFAULT_BINDINGS: Record<GlobalAction, string> = {
  playPause: "Ctrl+Alt+Space",
  next: "Ctrl+Alt+ArrowRight",
  previous: "Ctrl+Alt+ArrowLeft",
  volumeUp: "Ctrl+Alt+ArrowUp",
  volumeDown: "Ctrl+Alt+ArrowDown",
  like: "Ctrl+Alt+KeyL",
  showWindow: "Ctrl+Alt+KeyS",
};
