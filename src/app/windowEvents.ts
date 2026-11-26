// Window handlers: keyboard shortcuts, pasted link, WebView2's context menu, focus of the
// text fields on Android.

import type { Shell } from "./shell.svelte";
import { commands } from "../lib/ipc";
import { runAction } from "../lib/actions";
import { togglePalette } from "../lib/stores/palette.svelte";
import { trackInfo } from "../lib/stores/trackInfo.svelte";
import { menu } from "../lib/stores/menu.svelte";
import { picker } from "../lib/stores/playlistPicker.svelte";
import { platform } from "../lib/platform.svelte";
import { appWindow, setFullscreen } from "../lib/stores/window.svelte";
import { openTidalLink } from "../lib/links";
import { selection } from "../lib/stores/selection.svelte";

// Android: mini player and navigation hidden while typing, as in native apps; otherwise
// they would take the place of the results above the keyboard (#151).
const NOT_TEXT = new Set([
  "button",
  "checkbox",
  "color",
  "file",
  "radio",
  "range",
  "reset",
  "submit",
]);

function isTextField(el: EventTarget | null): boolean {
  if (el instanceof HTMLTextAreaElement) return true;
  return el instanceof HTMLInputElement && !NOT_TEXT.has(el.type);
}

export function onFocusChange(shell: Shell, e: FocusEvent) {
  if (!platform.mobile) return;
  shell.typing = isTextField(e.type === "focusin" ? e.target : e.relatedTarget);
}

export function onKeydown(shell: Shell, e: KeyboardEvent) {
  const target = e.target as HTMLElement;
  const typing = target.closest("input, textarea, [contenteditable]") !== null;
  // Window full screen (#131), separate from "Now playing"; Esc leaves it first, like a
  // video, then closes the view on the second press.
  if (e.key === "F11" && !platform.mobile) {
    e.preventDefault();
    runAction("window.fullscreen");
    return;
  }
  if (e.key === "Escape") {
    if (appWindow.fullscreen) setFullscreen(false);
    else if (shell.nowPlayingOpen) shell.nowPlayingOpen = false;
    // Dialogs, the palette and the menu handle their own Esc.
    else if (!menu.open && !picker.request && !trackInfo.track) selection.clear();
    return;
  }
  // Interface size (#337), as in a browser; Android has its own setting.
  if (e.ctrlKey && !platform.mobile && ["+", "=", "-", "0"].includes(e.key)) {
    e.preventDefault();
    runAction(
      e.key === "0" ? "window.zoomReset" : e.key === "-" ? "window.zoomOut" : "window.zoomIn",
    );
    return;
  }
  // Settings stay reachable when signed out (theme, cache).
  if (e.ctrlKey && e.key === ",") {
    e.preventDefault();
    runAction("go.settings");
    return;
  }
  if (!shell.session) return;
  // Desktop mini player (#384).
  if (e.ctrlKey && e.key.toLowerCase() === "m" && !platform.mobile) {
    e.preventDefault();
    runAction("window.mini");
    return;
  }
  // Command palette (#412) on desktop; Android keeps the Search page.
  if (e.ctrlKey && e.key.toLowerCase() === "k") {
    e.preventDefault();
    if (platform.mobile) runAction("go.search");
    else togglePalette();
  } else if (e.key === "/" && !typing) {
    e.preventDefault();
    runAction("go.search");
  } else if (
    e.ctrlKey &&
    e.key.toLowerCase() === "a" &&
    !typing &&
    !shell.nowPlayingOpen &&
    !menu.open
  ) {
    // Multiple selection (#332): the whole shown (filtered) list; without a track list on
    // the page, Ctrl+A keeps its usual meaning.
    if (selection.selectAll()) e.preventDefault();
  } else if (e.key === " " && !typing && !target.closest("button")) {
    e.preventDefault();
    // Not through `runAction`: with no track loaded, Space still goes to the backend.
    commands.togglePause();
  } else if (e.altKey && e.key === "ArrowLeft") {
    runAction("go.back");
  } else if (e.ctrlKey && (e.key === "ArrowUp" || e.key === "ArrowDown") && !typing) {
    e.preventDefault();
    runAction(e.key === "ArrowUp" ? "play.volumeUp" : "play.volumeDown");
  }
}

// No WebView2 menu (Back, Refresh, Print…) outside text fields and selections, where it is
// used to copy and paste. The app's menus call `preventDefault` before.
export function onContextmenu(e: MouseEvent) {
  if (e.defaultPrevented) return;
  const target = e.target as HTMLElement;
  const editable = target.closest("input, textarea, [contenteditable]") !== null;
  const selected = (window.getSelection()?.toString() ?? "") !== "";
  if (!editable && !selected) e.preventDefault();
}

// TIDAL link pasted outside a text field (#203): opens its page.
export function onPaste(shell: Shell, e: ClipboardEvent) {
  if (!shell.session) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest?.("input, textarea, [contenteditable]")) return;
  const text = e.clipboardData?.getData("text/plain");
  if (text) void openTidalLink(text);
}
