<script lang="ts">
  // Fond Risographie (#69) : papier grainé et formes géométriques tramées sur les bords,
  // imprimées en deux encres qui se superposent (multiplication) avec un léger décalage.
  // Rien n'est animé : une impression ne bouge pas.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <pattern id="riso-dots-a" width="9" height="9" patternUnits="userSpaceOnUse" patternTransform="rotate(15)">
        <circle cx="4.5" cy="4.5" r="2.6" style="fill: var(--ink-a)" />
      </pattern>
      <pattern id="riso-dots-b" width="8" height="8" patternUnits="userSpaceOnUse" patternTransform="rotate(-30)">
        <circle cx="4" cy="4" r="2.2" style="fill: var(--ink-b)" />
      </pattern>
      <filter id="riso-grain">
        <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="3" stitchTiles="stitch" />
        <feColorMatrix values="0 0 0 0 0.11  0 0 0 0 0.1  0 0 0 0 0.17  0 0 0 0.55 0" />
      </filter>
    </defs>

    <!-- Encre rose : grand cercle en haut à gauche, triangle en bas à droite. -->
    <g class="ink">
      <circle cx="120" cy="140" r="210" fill="url(#riso-dots-a)" />
      <path d="M1600 1000 L1600 560 L1240 1000 Z" fill="url(#riso-dots-a)" />
      <rect x="1440" y="90" width="120" height="120" style="fill: var(--ink-a)" opacity="0.85" />
    </g>

    <!-- Encre bleue, décalée de quelques pixels : demi-disque, bandes, petit cercle plein. -->
    <g class="ink">
      <path d="M0 1000 L0 700 A 300 300 0 0 1 300 1000 Z" fill="url(#riso-dots-b)" />
      <circle cx="132" cy="150" r="120" style="fill: var(--ink-b)" opacity="0.8" />
      <g style="fill: var(--ink-b)" opacity="0.75">
        <rect x="1380" y="250" width="200" height="14" transform="rotate(-8 1480 257)" />
        <rect x="1380" y="280" width="160" height="14" transform="rotate(-8 1460 287)" />
        <rect x="1380" y="310" width="120" height="14" transform="rotate(-8 1440 317)" />
      </g>
      <circle cx="1452" cy="104" r="40" style="fill: var(--ink-b)" opacity="0.9" />
    </g>

    <rect width="1600" height="1000" filter="url(#riso-grain)" opacity="0.18" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    --ink-a: color-mix(in srgb, var(--accent) 80%, #ff6fb5);
    --ink-b: var(--riso-blue);
  }

  svg {
    width: 100%;
    height: 100%;
  }

  /* Deux encres qui se superposent comme sur le tambour. */
  .ink {
    mix-blend-mode: multiply;
    opacity: 0.55;
  }

  .intense .ink {
    opacity: 0.4;
  }
</style>
