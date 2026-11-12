<script lang="ts">
  // Fond Désert (#339) : ciel pâle, soleil voilé, horizon bas, trois plans de dunes, rides du
  // sable au premier plan. Rien n'est animé, sauf une chose : à la fin d'un album, un minuscule
  // ver passe une fois dans l'horizon (bureau seulement, jamais en boucle).
  import { player } from "../../lib/stores/player.svelte";
  import { queue } from "../../lib/stores/queue.svelte";

  let { intense = false }: { intense?: boolean } = $props();

  /** Dernier titre d'un album lu jusqu'au bout : rien après lui dans la file ni de radio
   *  automatique, pas de répétition. Le passage lui-même attend les 30 dernières secondes
   *  (`.progress.ending`, voir desert.css). */
  const albumEnd = $derived(
    queue.view.source?.kind === "album" &&
      !player.state.hasNext &&
      player.state.repeat === "off" &&
      queue.view.manual.length === 0 &&
      queue.view.upcoming.length === 0,
  );

  // Rides du premier plan, générées une fois.
  const ripples = Array.from({ length: 9 }, (_, i) => {
    const y = 905 + i * 11;
    const a = 6 + i * 0.6;
    return `M0 ${y} Q200 ${y - a} 400 ${y} T800 ${y} T1200 ${y} T1600 ${y}`;
  });
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMax slice">
    <defs>
      <linearGradient id="ds-sky" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="var(--ds-sky)" />
        <stop offset="0.75" stop-color="#f1e2c6" />
        <stop offset="1" stop-color="var(--bg)" />
      </linearGradient>
    </defs>
    <rect width="1600" height="1000" fill="url(#ds-sky)" />

    <!-- Soleil voilé, haut et pâle. -->
    <circle class="sun" cx="1180" cy="260" r="70" />

    <!-- Horizon et dunes lointaines. -->
    <path class="far" d="M0 790 Q300 760 560 785 T1100 775 T1600 785 V1000 H0 Z" />

    <!-- Le ver : un dos qui affleure, puis replonge, posé sur l'horizon. -->
    <g class="worm" class:armed={albumEnd}>
      <path d="M0 784 q6 -10 14 -10 q8 0 12 6 q4 -8 10 -8 q6 0 8 12 z" />
    </g>

    <path class="mid" d="M0 845 Q260 800 520 838 T1040 826 T1600 842 V1000 H0 Z" />
    <path class="near" d="M0 905 Q340 860 760 895 T1600 885 V1000 H0 Z" />
    <g class="ripples">
      {#each ripples as d, i (i)}
        <path {d} />
      {/each}
    </g>
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }

  svg {
    width: 100%;
    height: 100%;
  }

  .sun {
    fill: #fbf3e2;
    opacity: 0.8;
  }

  .far {
    fill: var(--ds-dune);
    opacity: 0.7;
  }

  .mid {
    fill: var(--ds-ochre);
    opacity: 0.45;
  }

  .near {
    fill: var(--ds-ochre);
    opacity: 0.7;
  }

  .ripples path {
    fill: none;
    stroke: var(--ds-burnt);
    stroke-width: 1;
    opacity: 0.18;
  }

  /* Le ver ne se montre qu'armé (fin d'album) et pendant les 30 dernières secondes du titre ;
     il traverse une fois en 16 s puis reste hors champ. Jamais sur Android. */
  .worm {
    visibility: hidden;
    fill: var(--ds-burnt);
    opacity: 0.55;
  }

  :global(:root:not([data-platform="android"]):has(.progress.ending)) .worm.armed {
    visibility: visible;
    animation: ds-worm 16s linear forwards;
  }

  @keyframes ds-worm {
    from {
      transform: translateX(-60px);
    }
    to {
      transform: translateX(1660px);
    }
  }

  /* « En lecture » : les dunes restent, plus claires ; le soleil tombait sous les boutons. */
  .intense :is(.mid, .near) {
    opacity: 0.35;
  }

  .intense .sun {
    display: none;
  }
</style>
