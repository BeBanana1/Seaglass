// Raccourcis globaux (#413) : capture d'une combinaison au clavier et affichage. Logique pure,
// testée (`hotkeys.test.ts`). Le format (`Ctrl+Alt+KeyL`) est celui que lit `global-hotkey`
// côté Rust ; la touche est `KeyboardEvent.code`, indépendante de la disposition du clavier.

import type { GlobalAction } from "./ipc";

/** Ordre des actions dans les Réglages. */
export const GLOBAL_ACTIONS: GlobalAction[] = ["playPause", "next", "previous", "volumeUp", "volumeDown", "like", "showWindow"];

/** Touches que `global-hotkey` sait enregistrer (sans les modificateurs). */
const SUPPORTED =
  /^(Key[A-Z]|Digit[0-9]|Numpad([0-9]|Add|Decimal|Divide|Enter|Equal|Multiply|Subtract)|F([1-9]|1[0-9]|2[0-4])|Arrow(Up|Down|Left|Right)|Space|Enter|Tab|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Pause|PrintScreen|ScrollLock|Backquote|Backslash|BracketLeft|BracketRight|Comma|Equal|Minus|Period|Quote|Semicolon|Slash|AudioVolume(Up|Down|Mute)|Media(PlayPause|Stop|TrackNext|TrackPrevious))$/;

const MODIFIER = /^(Control|Alt|Shift|Meta|OS)(Left|Right)?$/;

export type Capture =
  /** Combinaison complète. */
  | { kind: "keys"; keys: string }
  /** Seulement des modificateurs pour l'instant : attendre la touche. */
  | { kind: "pending" }
  /** Pas de Ctrl, Alt ni Windows : la touche servirait à écrire ailleurs. */
  | { kind: "needsModifier" }
  | { kind: "unsupported" };

type KeyInfo = Pick<KeyboardEvent, "code" | "ctrlKey" | "altKey" | "shiftKey" | "metaKey">;

export function capture(e: KeyInfo): Capture {
  if (MODIFIER.test(e.code)) return { kind: "pending" };
  if (!SUPPORTED.test(e.code)) return { kind: "unsupported" };
  if (!e.ctrlKey && !e.altKey && !e.metaKey) return { kind: "needsModifier" };
  const parts = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift", e.metaKey && "Super", e.code];
  return { kind: "keys", keys: parts.filter(Boolean).join("+") };
}

/** Même combinaison écrite autrement (casse, ordre des modificateurs) ? */
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

/** Texte affiché : `Ctrl+Alt+KeyL` → `Ctrl + Alt + L`, `Ctrl+Alt+ArrowUp` → `Ctrl + Alt + ↑`.
 *  `space` : nom de la barre d'espace dans la langue courante. */
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

/** Combinaisons proposées, les mêmes que `GlobalShortcuts::default` côté Rust. */
export const DEFAULT_BINDINGS: Record<GlobalAction, string> = {
  playPause: "Ctrl+Alt+Space",
  next: "Ctrl+Alt+ArrowRight",
  previous: "Ctrl+Alt+ArrowLeft",
  volumeUp: "Ctrl+Alt+ArrowUp",
  volumeDown: "Ctrl+Alt+ArrowDown",
  like: "Ctrl+Alt+KeyL",
  showWindow: "Ctrl+Alt+KeyS",
};
