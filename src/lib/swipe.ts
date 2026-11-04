// Glisser une ligne de titre vers la droite (Android, #174) : ajoute le titre à la file.
// Volontairement peu sensible, pour qu'un défilement ne déclenche jamais rien :
// - `touch-action: pan-y` (CSS de la ligne) : le défilement vertical reste au navigateur, qui
//   annule le geste (`pointercancel`) dès qu'il le prend ;
// - direction décidée après LOCK px, geste abandonné s'il n'est pas nettement horizontal ;
// - déclenché seulement au-delà d'une part de la largeur de la ligne, au relâcher.
// Le déplacement passe par une variable CSS et des classes posées ici, sans état Svelte : rien
// n'est recalculé à chaque image du doigt.

/** Distance avant de décider de la direction (px). */
const LOCK = 12;
/** Le geste doit être au moins DOMINANCE fois plus horizontal que vertical. */
const DOMINANCE = 2;
/** Bord gauche réservé au Retour d'Android en navigation par gestes (px). */
const EDGE = 24;
/** Seuil de déclenchement : part de la largeur de la ligne, avec un minimum en px. */
const RATIO = 0.35;
const MIN_DISTANCE = 96;
/** Durée du retour de la ligne à sa place (ms), comme la transition CSS. */
const RELEASE_MS = 180;

export interface SwipeOptions {
  enabled: boolean;
  onSwipe: () => void;
}

export function swipeRight(node: HTMLElement, options: SwipeOptions) {
  let opts = options;
  let pointer: number | null = null;
  let phase: "idle" | "pending" | "swiping" = "idle";
  let x0 = 0;
  let y0 = 0;
  let dx = 0;
  let threshold = MIN_DISTANCE;
  /** Un glissement vient de finir : le clic qui suit n'est pas un appui sur la ligne. */
  let swallowClick = false;
  let releaseTimer: ReturnType<typeof setTimeout> | undefined;

  function stop() {
    pointer = null;
    phase = "idle";
  }

  function release() {
    node.classList.remove("swipe-armed");
    node.classList.add("swipe-release");
    node.style.setProperty("--swipe-x", "0px");
    node.style.setProperty("--swipe-fade", "0");
    clearTimeout(releaseTimer);
    releaseTimer = setTimeout(() => {
      node.classList.remove("swiping", "swipe-release");
      node.style.removeProperty("--swipe-x");
      node.style.removeProperty("--swipe-fade");
    }, RELEASE_MS);
  }

  function onDown(e: PointerEvent) {
    if (!opts.enabled || !e.isPrimary || e.button !== 0) return;
    if (e.clientX < EDGE) return;
    pointer = e.pointerId;
    phase = "pending";
    x0 = e.clientX;
    y0 = e.clientY;
    dx = 0;
    threshold = Math.max(MIN_DISTANCE, node.getBoundingClientRect().width * RATIO);
  }

  function onMove(e: PointerEvent) {
    if (e.pointerId !== pointer || phase === "idle") return;
    const mx = e.clientX - x0;
    const my = e.clientY - y0;
    if (phase === "pending") {
      if (Math.hypot(mx, my) < LOCK) return;
      if (mx <= 0 || mx < DOMINANCE * Math.abs(my)) return stop();
      phase = "swiping";
      // Garde le doigt même s'il sort de la ligne ; échoue si le pointeur n'est plus actif.
      try {
        node.setPointerCapture(e.pointerId);
      } catch {
        /* sans capture, le geste continue tant que le doigt reste sur la ligne */
      }
      clearTimeout(releaseTimer);
      node.classList.remove("swipe-release");
      node.classList.add("swiping");
    }
    dx = Math.max(0, mx);
    node.style.setProperty("--swipe-x", `${dx}px`);
    node.style.setProperty("--swipe-fade", String(Math.min(1, dx / 48)));
    node.classList.toggle("swipe-armed", dx >= threshold);
  }

  function onUp(e: PointerEvent) {
    if (e.pointerId !== pointer) return;
    const swiped = phase === "swiping";
    const fire = swiped && dx >= threshold;
    stop();
    if (!swiped) return;
    release();
    // Le clic éventuel arrive juste après `pointerup`, dans la même tâche.
    swallowClick = true;
    setTimeout(() => (swallowClick = false), 0);
    if (fire) opts.onSwipe();
  }

  function onCancel(e: PointerEvent) {
    if (e.pointerId !== pointer) return;
    const swiped = phase === "swiping";
    stop();
    if (swiped) release();
  }

  // Phase de capture : avant les gestionnaires de la ligne et de ses boutons.
  function onClick(e: MouseEvent) {
    if (!swallowClick) return;
    swallowClick = false;
    e.preventDefault();
    e.stopPropagation();
  }

  function onContextmenu(e: MouseEvent) {
    if (phase !== "swiping") return;
    e.preventDefault();
    e.stopPropagation();
  }

  node.addEventListener("pointerdown", onDown);
  node.addEventListener("pointermove", onMove);
  node.addEventListener("pointerup", onUp);
  node.addEventListener("pointercancel", onCancel);
  node.addEventListener("click", onClick, true);
  node.addEventListener("contextmenu", onContextmenu, true);

  return {
    update(next: SwipeOptions) {
      opts = next;
      if (!opts.enabled && phase !== "idle") {
        const swiped = phase === "swiping";
        stop();
        if (swiped) release();
      }
    },
    destroy() {
      clearTimeout(releaseTimer);
      node.removeEventListener("pointerdown", onDown);
      node.removeEventListener("pointermove", onMove);
      node.removeEventListener("pointerup", onUp);
      node.removeEventListener("pointercancel", onCancel);
      node.removeEventListener("click", onClick, true);
      node.removeEventListener("contextmenu", onContextmenu, true);
    },
  };
}
