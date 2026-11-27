<script lang="ts">
  // Generique backdrop (#339): large paper cutout shapes laid askew on black paper
  // (orange, red, cream), a broken line running across, a crisp drop shadow under
  // each cutout. Asymmetrical, nothing aligned. Nothing is animated.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <filter id="gq-paper">
        <feTurbulence type="fractalNoise" baseFrequency="0.7" numOctaves="2" seed="5" />
        <feColorMatrix type="saturate" values="0" />
        <feComponentTransfer><feFuncA type="table" tableValues="0 0.08" /></feComponentTransfer>
        <feComposite in2="SourceGraphic" operator="in" />
      </filter>
    </defs>

    <!-- Large orange cutout on the right, running off the frame; below the header band,
         where it made the links unreadable. -->
    <g class="cut orange" transform="translate(0 190)">
      <polygon
        class="shadow"
        points="1090,-30 1640,-20 1630,380 1210,330 1150,190"
        transform="translate(10 12)"
      />
      <polygon points="1090,-30 1640,-20 1630,380 1210,330 1150,190" />
    </g>

    <!-- Red band at an angle, at the bottom left. -->
    <g class="cut red">
      <polygon
        class="shadow"
        points="-40,820 520,742 560,800 -20,905"
        transform="translate(8 10)"
      />
      <polygon points="-40,820 520,742 560,800 -20,905" />
    </g>

    <!-- Small badly cut cream circle. -->
    <g class="cut cream">
      <path
        class="shadow"
        d="M1384 640 l26 -6 22 14 10 26 -6 26 -22 18 -28 2 -24 -14 -8 -28 8 -24 z"
        transform="translate(6 8)"
      />
      <path d="M1384 640 l26 -6 22 14 10 26 -6 26 -22 18 -28 2 -24 -14 -8 -28 8 -24 z" />
    </g>

    <!-- Cream broken line running across the screen. -->
    <polyline
      class="broken"
      points="-20,560 240,590 330,520 610,610 760,540 1010,600 1120,560 1620,640"
    />

    <rect width="1600" height="1000" filter="url(#gq-paper)" fill="white" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: var(--bg);
  }

  svg {
    width: 100%;
    height: 100%;
  }

  .shadow {
    fill: #000;
    opacity: 0.55;
  }

  .orange polygon:not(.shadow) {
    fill: var(--gq-orange);
  }

  .red polygon:not(.shadow) {
    fill: var(--gq-red);
  }

  .cream path:not(.shadow) {
    fill: var(--gq-cream);
  }

  .broken {
    fill: none;
    stroke: var(--gq-cream);
    stroke-width: 3;
    stroke-linejoin: miter;
    opacity: 0.35;
  }

  /* "Now playing": the cutouts pass behind the cover and the queue, more muted. */
  .intense .cut {
    opacity: 0.45;
  }

  /* The large orange cutout used to fall under the duration and the corner buttons. */
  .intense .orange {
    display: none;
  }

  .intense .broken {
    opacity: 0.18;
  }
</style>
