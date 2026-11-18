// Glisser au doigt pour réorganiser une liste (Android, #290) : le glisser HTML5 ne marche
// pas au toucher dans la WebView. Un appui long (HOLD ms sans bouger) saisit la ligne, qui
// suit alors le doigt ; le repère d'insertion et le déplacement final passent par le même
// `Reorder` que le glisser de la souris.
// - Avant la saisie, un déplacement de plus de SLOP px rend la main au défilement.
// - Après la saisie, `touchmove` est annulé (écouteur non passif) : la page ne défile plus
//   sous le doigt ; elle défile seule quand le doigt approche d'un bord du conteneur, mais
//   seulement tant qu'il reste de la liste à montrer de ce côté (#302).
// - La ligne saisie reste dans la liste : sinon son `transform` agrandissait la zone
//   défilable et le défilement automatique descendait sans fin sous la liste (#302).
// - Un appui qui commence sur un bouton n'est pas une saisie (Monter / Descendre, Masquer).
// - Avec une poignée (`handle`, file d'attente #450), seul un appui sur elle saisit la ligne,
//   et tout de suite : l'appui long sur le reste de la ligne garde son menu. La poignée porte
//   `touch-action: none`, la page ne défile donc pas sous elle.
// La ligne suit le doigt par une variable CSS et des classes posées ici, sans état Svelte.

import type { Reorder } from "./reorder.svelte";

/** Durée de l'appui avant la saisie (ms). */
const HOLD = 350;
/** Tolérance de mouvement du doigt pendant l'appui (px). */
const SLOP = 10;
/** Zone près des bords du conteneur où la page défile seule (px). */
const EDGE = 64;
/** Vitesse maximale du défilement automatique (px par image). */
const SPEED = 14;

export interface TouchReorderOptions {
  enabled: boolean;
  /** Position de la ligne dans la liste réorganisable. */
  index: number;
  reorder: Reorder;
  /** Lignes de la liste, dans l'ordre, pour situer le doigt. */
  items: () => HTMLElement[];
  /** Sélecteur de la poignée : l'appui doit y commencer, et saisit sans attendre. */
  handle?: string;
}

function scrollParent(node: HTMLElement): HTMLElement {
  for (let el = node.parentElement; el; el = el.parentElement) {
    const { overflowY } = getComputedStyle(el);
    if ((overflowY === "auto" || overflowY === "scroll") && el.scrollHeight > el.clientHeight)
      return el;
  }
  return (document.scrollingElement as HTMLElement) ?? document.documentElement;
}

export function touchReorder(node: HTMLElement, options: TouchReorderOptions) {
  let opts = options;
  let pointer: number | null = null;
  let phase: "idle" | "pending" | "dragging" = "idle";
  let timer: ReturnType<typeof setTimeout> | undefined;
  let x0 = 0;
  let y0 = 0;
  let y = 0;
  let scroller: HTMLElement | null = null;
  let scroll0 = 0;
  let frame = 0;
  /** Décalage appliqué à la ligne saisie (`--drag-y`), en px. */
  let shift = 0;
  /** Une saisie vient de finir : le clic qui suit n'est pas un appui sur la ligne. */
  let swallowClick = false;

  /** Haut et bas de la liste à l'écran, la ligne saisie comptée à sa place d'origine. */
  function bounds(items: HTMLElement[]): { top: number; bottom: number } | null {
    if (items.length === 0) return null;
    const rect = (el: HTMLElement) => (el === node ? originRect() : el.getBoundingClientRect());
    const first = rect(items[0]);
    const last = rect(items[items.length - 1]);
    return { top: first.top, bottom: last.top + last.height };
  }

  function place() {
    if (!scroller) return;
    const items = opts.items();
    const origin = originRect();
    let next = y - y0 + scroller.scrollTop - scroll0;
    const list = bounds(items);
    if (list) {
      next = Math.min(next, list.bottom - origin.top - origin.height);
      next = Math.max(next, list.top - origin.top);
    }
    shift = next;
    node.style.setProperty("--drag-y", `${shift}px`);
    // Emplacement : avant la première ligne dont le milieu est sous le doigt.
    let slot = items.length;
    for (let i = 0; i < items.length; i++) {
      // La ligne saisie est déplacée par `transform` : on la situe à sa place d'origine.
      const rect = items[i] === node ? originRect() : items[i].getBoundingClientRect();
      if (y < rect.top + rect.height / 2) {
        slot = i;
        break;
      }
    }
    opts.reorder.slot = slot;
  }

  function originRect(): DOMRect {
    const r = node.getBoundingClientRect();
    return new DOMRect(r.x, r.y - shift, r.width, r.height);
  }

  function autoScroll() {
    frame = 0;
    if (phase !== "dragging" || !scroller) return;
    const box =
      scroller === document.scrollingElement
        ? { top: 0, bottom: window.innerHeight }
        : scroller.getBoundingClientRect();
    let step = 0;
    if (y < box.top + EDGE) step = -SPEED * Math.min(1, (box.top + EDGE - y) / EDGE);
    else if (y > box.bottom - EDGE) step = SPEED * Math.min(1, (y - (box.bottom - EDGE)) / EDGE);
    // Pas au-delà de la liste : on s'arrête quand son bout est sorti de la zone du bord.
    const list = bounds(opts.items());
    if (list && step > 0) step = Math.min(step, Math.max(0, list.bottom - (box.bottom - EDGE)));
    if (list && step < 0) step = Math.max(step, Math.min(0, list.top - (box.top + EDGE)));
    if (step !== 0) {
      const before = scroller.scrollTop;
      scroller.scrollTop += step;
      if (scroller.scrollTop !== before) place();
    }
    frame = requestAnimationFrame(autoScroll);
  }

  function grab() {
    phase = "dragging";
    shift = 0;
    scroller = scrollParent(node);
    scroll0 = scroller.scrollTop;
    node.classList.add("touch-dragging");
    opts.reorder.from = opts.index;
    navigator.vibrate?.(15);
    place();
    frame = requestAnimationFrame(autoScroll);
  }

  function finish(commit: boolean) {
    clearTimeout(timer);
    if (phase === "dragging") {
      if (commit) opts.reorder.commit();
      else opts.reorder.end();
      node.classList.remove("touch-dragging");
      node.style.removeProperty("--drag-y");
      shift = 0;
      swallowClick = true;
      setTimeout(() => (swallowClick = false), 0);
    }
    cancelAnimationFrame(frame);
    frame = 0;
    pointer = null;
    phase = "idle";
    scroller = null;
  }

  function onDown(e: PointerEvent) {
    if (!opts.enabled || !e.isPrimary || e.pointerType === "mouse") return;
    const target = e.target as Element;
    if (opts.handle ? !target.closest(opts.handle) : target.closest("button, a, input")) return;
    pointer = e.pointerId;
    phase = "pending";
    x0 = e.clientX;
    y0 = y = e.clientY;
    clearTimeout(timer);
    if (opts.handle) grab();
    else timer = setTimeout(grab, HOLD);
  }

  function onMove(e: PointerEvent) {
    if (e.pointerId !== pointer) return;
    if (phase === "pending") {
      if (Math.hypot(e.clientX - x0, e.clientY - y0) > SLOP) finish(false);
      return;
    }
    y = e.clientY;
    place();
  }

  function onUp(e: PointerEvent) {
    if (e.pointerId === pointer) finish(true);
  }

  function onCancel(e: PointerEvent) {
    if (e.pointerId === pointer) finish(false);
  }

  /** Après la saisie, le doigt déplace la ligne et non la page. */
  function onTouchMove(e: TouchEvent) {
    if (phase === "dragging" && e.cancelable) e.preventDefault();
  }

  /** L'appui long ouvrirait sinon le menu ou la sélection de texte de la WebView. */
  function onContextMenu(e: Event) {
    if (phase !== "idle") e.preventDefault();
  }

  function onClick(e: MouseEvent) {
    if (!swallowClick) return;
    e.preventDefault();
    e.stopPropagation();
  }

  node.addEventListener("pointerdown", onDown);
  node.addEventListener("pointermove", onMove);
  node.addEventListener("pointerup", onUp);
  node.addEventListener("pointercancel", onCancel);
  node.addEventListener("touchmove", onTouchMove, { passive: false });
  node.addEventListener("contextmenu", onContextMenu);
  node.addEventListener("click", onClick, true);

  return {
    update(next: TouchReorderOptions) {
      opts = next;
      if (!opts.enabled && phase !== "idle") finish(false);
    },
    destroy() {
      finish(false);
      node.removeEventListener("pointerdown", onDown);
      node.removeEventListener("pointermove", onMove);
      node.removeEventListener("pointerup", onUp);
      node.removeEventListener("pointercancel", onCancel);
      node.removeEventListener("touchmove", onTouchMove);
      node.removeEventListener("contextmenu", onContextMenu);
      node.removeEventListener("click", onClick, true);
    },
  };
}
