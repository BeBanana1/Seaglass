// Gestionnaires de la fenêtre : raccourcis clavier, lien collé, menu contextuel de WebView2,
// focus des champs sur Android.

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

// Android : mini-lecteur et navigation masqués pendant la saisie, comme dans les apps
// natives ; ils prendraient sinon la place des résultats au-dessus du clavier (#151).
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
  // Plein écran de la fenêtre (#131), indépendant de « En lecture » ; Échap en sort
  // d'abord, comme une vidéo, puis ferme la vue au second appui.
  if (e.key === "F11" && !platform.mobile) {
    e.preventDefault();
    runAction("window.fullscreen");
    return;
  }
  if (e.key === "Escape") {
    if (appWindow.fullscreen) setFullscreen(false);
    else if (shell.nowPlayingOpen) shell.nowPlayingOpen = false;
    // Les dialogues, la palette et le menu gèrent leur propre Échap.
    else if (!menu.open && !picker.request && !trackInfo.track) selection.clear();
    return;
  }
  // Taille de l'interface (#337), comme dans un navigateur ; Android a son propre réglage.
  if (e.ctrlKey && !platform.mobile && ["+", "=", "-", "0"].includes(e.key)) {
    e.preventDefault();
    runAction(
      e.key === "0" ? "window.zoomReset" : e.key === "-" ? "window.zoomOut" : "window.zoomIn",
    );
    return;
  }
  // Les réglages restent accessibles déconnecté (thème, cache).
  if (e.ctrlKey && e.key === ",") {
    e.preventDefault();
    runAction("go.settings");
    return;
  }
  if (!shell.session) return;
  // Mini-lecteur du bureau (#384).
  if (e.ctrlKey && e.key.toLowerCase() === "m" && !platform.mobile) {
    e.preventDefault();
    runAction("window.mini");
    return;
  }
  // Palette de commandes (#412) sur le bureau ; Android garde la page Recherche.
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
    // Sélection multiple (#332) : toute la liste affichée (filtrée) ; sans liste de titres
    // sur la page, Ctrl+A garde son sens habituel.
    if (selection.selectAll()) e.preventDefault();
  } else if (e.key === " " && !typing && !target.closest("button")) {
    e.preventDefault();
    // Pas par `runAction` : sans titre chargé, Espace part quand même au backend.
    commands.togglePause();
  } else if (e.altKey && e.key === "ArrowLeft") {
    runAction("go.back");
  } else if (e.ctrlKey && (e.key === "ArrowUp" || e.key === "ArrowDown") && !typing) {
    e.preventDefault();
    runAction(e.key === "ArrowUp" ? "play.volumeUp" : "play.volumeDown");
  }
}

// Pas de menu WebView2 (Précédent, Actualiser, Imprimer…) hors des champs de texte et des
// sélections, où il sert à copier-coller. Les menus de l'app appellent `preventDefault` avant.
export function onContextmenu(e: MouseEvent) {
  if (e.defaultPrevented) return;
  const target = e.target as HTMLElement;
  const editable = target.closest("input, textarea, [contenteditable]") !== null;
  const selected = (window.getSelection()?.toString() ?? "") !== "";
  if (!editable && !selected) e.preventDefault();
}

// Lien TIDAL collé hors d'un champ de texte (#203) : ouvre sa page.
export function onPaste(shell: Shell, e: ClipboardEvent) {
  if (!shell.session) return;
  const target = e.target as HTMLElement | null;
  if (target?.closest?.("input, textarea, [contenteditable]")) return;
  const text = e.clipboardData?.getData("text/plain");
  if (text) void openTidalLink(text);
}
