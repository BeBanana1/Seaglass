<script lang="ts">
  // Bauhaus backdrop (#70): fixed geometric composition on the edges (disc, half disc,
  // bars, grid hairlines). The quarter circle and the right bar rotate very
  // slowly during playback (`data-playing` on <html>).
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <!-- Grid hairlines. -->
    <g stroke="#141414" stroke-width="1" opacity="0.12">
      <path d="M400 0 V1000 M800 0 V1000 M1200 0 V1000 M0 250 H1600 M0 500 H1600 M0 750 H1600" />
    </g>

    <!-- Red disc on the right, half off-screen; the top band stays free for the header. -->
    <circle cx="1560" cy="560" r="210" style="fill: var(--red)" />

    <!-- Blue quarter circle and black bar rotating around the same point. -->
    <g class="turn" style="transform-origin: 1360px 330px">
      <path d="M1360 330 L1560 330 A 200 200 0 0 0 1360 130 Z" style="fill: var(--blue)" />
      <rect x="1250" y="322" width="320" height="16" fill="#141414" />
    </g>

    <!-- Yellow half disc at the bottom left, blue square, red triangle. -->
    <path d="M-40 1000 A 260 260 0 0 1 480 1000 Z" style="fill: var(--yellow)" />
    <rect x="40" y="610" width="110" height="110" style="fill: var(--blue)" />
    <path d="M170 300 L300 300 L170 450 Z" style="fill: var(--red)" />

    <!-- Poster bars. -->
    <rect x="1300" y="840" width="300" height="22" fill="#141414" />
    <rect x="1380" y="880" width="220" height="22" style="fill: var(--yellow)" />
    <circle cx="320" cy="560" r="26" fill="none" stroke="#141414" stroke-width="6" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    --red: var(--accent);
    --blue: var(--bh-blue);
    --yellow: var(--bh-yellow);
  }

  svg {
    width: 100%;
    height: 100%;
  }

  .intense svg {
    opacity: 0.55;
  }

  :global(:root[data-playing="true"]) .turn {
    animation: turn 80s linear infinite;
  }

  @keyframes turn {
    to {
      transform: rotate(360deg);
    }
  }
</style>
