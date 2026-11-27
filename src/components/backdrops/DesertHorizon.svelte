<script lang="ts">
  // Desert backdrop (#339): pale sky, veiled sun, low horizon, three planes of dunes, sand
  // ripples in the foreground. Nothing is animated, except one thing: at the end of an album,
  // a tiny worm crosses the horizon once (desktop only, never in a loop).
  import { player } from "../../lib/stores/player.svelte";
  import { queue } from "../../lib/stores/queue.svelte";

  let { intense = false }: { intense?: boolean } = $props();

  /** Last track of an album played to the end: nothing after it in the queue, no automatic
   *  radio and no repeat. The crossing itself waits for the last 30 seconds
   *  (`.progress.ending`, see desert.css). */
  const albumEnd = $derived(
    queue.view.source?.kind === "album" &&
      !player.state.hasNext &&
      player.state.repeat === "off" &&
      queue.view.manual.length === 0 &&
      queue.view.upcoming.length === 0,
  );

  // Foreground ripples, generated once.
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

    <!-- Veiled sun, high and pale. -->
    <circle class="sun" cx="1180" cy="260" r="70" />

    <!-- Horizon and distant dunes. -->
    <path class="far" d="M0 790 Q300 760 560 785 T1100 775 T1600 785 V1000 H0 Z" />

    <!-- The worm: a back that surfaces, then dives again, resting on the horizon. -->
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

  /* The worm only shows when armed (end of album) and during the last 30 seconds of the
     track; it crosses once in 16 s then stays off-screen. Never on Android. */
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

  /* "Now playing": the dunes stay, lighter; the sun used to fall under the buttons. */
  .intense :is(.mid, .near) {
    opacity: 0.35;
  }

  .intense .sun {
    display: none;
  }
</style>
