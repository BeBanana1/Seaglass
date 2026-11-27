<script lang="ts">
  // Cassette backdrop (#338): dark walnut (stretched `feTurbulence` grain), crossed by the
  // wide bands of a cassette sleeve (orange, mustard, olive) and a cream hairline.
  // Nothing is animated.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <filter id="cs-grain">
        <feTurbulence type="fractalNoise" baseFrequency="0.002 0.06" numOctaves="3" seed="4" />
        <feColorMatrix values="0 0 0 0 0.05  0 0 0 0 0.03  0 0 0 0 0.01  0 0 0 1.1 -0.2" />
      </filter>
    </defs>

    <rect class="wood" width="1600" height="1000" />
    <rect width="1600" height="1000" filter="url(#cs-grain)" opacity="0.7" />

    <g class="band">
      <rect y="640" width="1600" height="34" style="fill: var(--cs-orange)" />
      <rect y="674" width="1600" height="34" style="fill: var(--cs-mustard)" />
      <rect y="708" width="1600" height="34" style="fill: var(--cs-olive)" />
      <rect y="754" width="1600" height="4" style="fill: var(--text)" opacity="0.6" />
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

  .wood {
    fill: var(--bg);
  }

  .band {
    opacity: 0.85;
  }

  /* "Now playing": the band passes behind the queue and the controls, more muted. */
  .intense .band {
    opacity: 0.35;
  }
</style>
