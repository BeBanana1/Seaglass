<script lang="ts">
  // Film noir backdrop (#339): window light passes through a Venetian blind and
  // falls at an angle on the wall (light slats in a softened parallelogram). Film grain,
  // fixed. In "Now playing", still smoke rises from the bottom as a watermark. Nothing moves.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <div class="blinds"></div>
  {#if intense}
    <svg class="smoke" viewBox="0 0 600 900" preserveAspectRatio="xMidYMax slice">
      <defs>
        <filter id="nr-smoke" x="-20%" y="-20%" width="140%" height="140%">
          <feTurbulence type="fractalNoise" baseFrequency="0.006 0.018" numOctaves="4" seed="11" />
          <feDisplacementMap in="SourceGraphic" scale="140" />
          <feGaussianBlur stdDeviation="14" />
        </filter>
        <linearGradient id="nr-smoke-fade" x1="0" y1="1" x2="0" y2="0">
          <stop offset="0" stop-color="#fff" stop-opacity="0.5" />
          <stop offset="1" stop-color="#fff" stop-opacity="0" />
        </linearGradient>
      </defs>
      <path
        d="M300 900 C 260 760, 360 680, 300 560 S 220 380, 320 260 S 380 80, 300 0"
        fill="none"
        stroke="url(#nr-smoke-fade)"
        stroke-width="110"
        filter="url(#nr-smoke)"
      />
    </svg>
  {/if}
  <svg class="grain" width="100%" height="100%">
    <filter id="nr-grain">
      <feTurbulence
        type="fractalNoise"
        baseFrequency="0.85"
        numOctaves="2"
        seed="3"
        stitchTiles="stitch"
      />
      <feColorMatrix type="saturate" values="0" />
    </filter>
    <rect width="100%" height="100%" filter="url(#nr-grain)" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: radial-gradient(ellipse 70% 60% at 70% 30%, #1b1b1b, var(--bg) 75%);
  }

  /* Blind slats: light bands, projected at an angle, edges softened by a mask. */
  .blinds {
    position: absolute;
    top: -20%;
    right: -10%;
    width: 75%;
    height: 120%;
    background: repeating-linear-gradient(
      180deg,
      rgb(255 255 255 / 0.075) 0 22px,
      transparent 22px 40px
    );
    transform: skewY(-18deg) rotate(-8deg);
    transform-origin: top right;
    -webkit-mask: radial-gradient(ellipse 60% 55% at 60% 45%, black 30%, transparent 75%);
    mask: radial-gradient(ellipse 60% 55% at 60% 45%, black 30%, transparent 75%);
  }

  .smoke {
    position: absolute;
    left: 2%;
    bottom: 0;
    width: min(40vw, 520px);
    height: 90%;
    opacity: 0.16;
  }

  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.07;
    mix-blend-mode: screen;
  }

  .intense .blinds {
    opacity: 0.7;
  }
</style>
