<script lang="ts">
  // Fond Solarpunk (#65) : ville végétalisée au loin, soleil diffus, éolienne, fougères et
  // vrilles sur les bords. Le centre reste au contenu. Les feuilles ne se balancent que
  // pendant la lecture (`data-playing` sur <html>).
  let { intense = false }: { intense?: boolean } = $props();

  type Pt = [number, number];

  /** Fronde le long d'une courbe de Bézier quadratique : folioles de part et d'autre de la tige. */
  function frond(p0: Pt, p1: Pt, p2: Pt, size: number, count: number, sides: 1 | 2 = 2) {
    const leaves: { x: number; y: number; a: number; s: number }[] = [];
    for (let i = 1; i <= count; i++) {
      const t = i / (count + 1);
      const u = 1 - t;
      const x = u * u * p0[0] + 2 * u * t * p1[0] + t * t * p2[0];
      const y = u * u * p0[1] + 2 * u * t * p1[1] + t * t * p2[1];
      const dx = 2 * u * (p1[0] - p0[0]) + 2 * t * (p2[0] - p1[0]);
      const dy = 2 * u * (p1[1] - p0[1]) + 2 * t * (p2[1] - p1[1]);
      const a = (Math.atan2(dy, dx) * 180) / Math.PI;
      const s = size * (1 - t * 0.65);
      if (sides === 2) {
        leaves.push({ x, y, a: a - 52, s }, { x, y, a: a + 52, s });
      } else {
        leaves.push({ x, y, a: a + (i % 2 ? -58 : 58), s });
      }
    }
    return { d: `M${p0[0]} ${p0[1]} Q${p1[0]} ${p1[1]} ${p2[0]} ${p2[1]}`, origin: p0, leaves };
  }

  const fronds = [
    { ...frond([-20, 1010], [60, 760], [260, 640], 0.95, 12), tone: 1, delay: 0 },
    { ...frond([-10, 1000], [-20, 700], [90, 470], 0.85, 11), tone: 2, delay: -2 },
    { ...frond([40, 1010], [220, 880], [430, 860], 0.7, 10), tone: 3, delay: -4 },
    { ...frond([1620, 1010], [1540, 780], [1350, 660], 0.95, 12), tone: 1, delay: -1 },
    { ...frond([1615, 990], [1640, 700], [1520, 480], 0.85, 11), tone: 2, delay: -3 },
    { ...frond([1600, 1000], [1470, 900], [1220, 880], 0.7, 10), tone: 3, delay: -5 },
  ];

  const vines = [
    { ...frond([-10, -10], [170, 120], [110, 360], 0.55, 9, 1), delay: 0 },
    { ...frond([380, -10], [430, 80], [360, 190], 0.4, 6, 1), delay: -3 },
  ];

  // Tours de la ville au loin : sommet en arche, terrasses plantées, panneaux solaires.
  const towers = [
    { x: 420, w: 70, h: 150 },
    { x: 510, w: 54, h: 200 },
    { x: 590, w: 90, h: 120 },
    { x: 760, w: 64, h: 220 },
    { x: 850, w: 80, h: 160 },
    { x: 1010, w: 58, h: 190 },
    { x: 1090, w: 96, h: 130 },
    { x: 1220, w: 60, h: 170 },
  ];
  const ground = 820;
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMax slice">
    <defs>
      <linearGradient id="sol-sky" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" style="stop-color: var(--sky-top)" />
        <stop offset="0.75" style="stop-color: var(--sky-low)" />
      </linearGradient>
      <radialGradient id="sol-sun" cx="0.5" cy="0.5" r="0.5">
        <stop offset="0" stop-color="#fff6cf" stop-opacity="0.95" />
        <stop offset="0.35" style="stop-color: var(--glow)" stop-opacity="0.55" />
        <stop offset="1" style="stop-color: var(--glow)" stop-opacity="0" />
      </radialGradient>
      <path id="sol-leaf" d="M0 0 C 18 -16 52 -18 78 0 C 52 18 18 16 0 0 Z" />
      <path id="sol-broad" d="M0 0 C 30 -64 128 -74 178 -12 C 126 42 42 42 0 0 Z" />
      <filter id="sol-grain">
        <feTurbulence type="fractalNoise" baseFrequency="0.85" numOctaves="2" stitchTiles="stitch" />
        <feColorMatrix values="0 0 0 0 0.12  0 0 0 0 0.22  0 0 0 0 0.12  0 0 0 0.45 0" />
      </filter>
    </defs>

    <rect width="1600" height="1000" fill="url(#sol-sky)" />

    <!-- Soleil diffus et halos concentriques, à la manière d'un vitrail. -->
    <circle cx="1240" cy="210" r="330" fill="url(#sol-sun)" />
    <g class="halo" fill="none" style="stroke: var(--gold)">
      <circle cx="1240" cy="210" r="120" stroke-width="2" opacity="0.28" />
      <circle cx="1240" cy="210" r="175" stroke-width="1.5" opacity="0.18" />
      <circle cx="1240" cy="210" r="240" stroke-width="1" opacity="0.12" />
    </g>

    <!-- Ville au loin. -->
    <g class="city">
      {#each towers as tower (tower.x)}
        {@const top = ground - tower.h}
        <rect x={tower.x} y={top} width={tower.w} height={tower.h + 40} rx={tower.w / 2} style="fill: var(--city)" />
        <path
          d="M{tower.x + 8} {top + tower.w * 0.5 + 24} l{tower.w - 16} -10 v8 l-{tower.w - 16} 10 Z"
          style="fill: var(--panel)"
        />
        <circle cx={tower.x + tower.w * 0.3} cy={top + tower.w * 0.5 + 70} r="12" style="fill: var(--leaf-3)" />
        <circle cx={tower.x + tower.w * 0.68} cy={top + tower.w * 0.5 + 64} r="15" style="fill: var(--leaf-2)" />
      {/each}
    </g>

    <!-- Éolienne à gauche. -->
    <g class="turbine">
      <path d="M248 820 L252 820 L251 540 L249 540 Z" style="fill: var(--city-dark)" />
      <g class="blades">
        <path d="M250 540 L246 470 L250 440 L254 470 Z" style="fill: var(--city-dark)" />
        <path d="M250 540 L246 470 L250 440 L254 470 Z" style="fill: var(--city-dark)" transform="rotate(120 250 540)" />
        <path d="M250 540 L246 470 L250 440 L254 470 Z" style="fill: var(--city-dark)" transform="rotate(240 250 540)" />
      </g>
      <circle cx="250" cy="540" r="6" style="fill: var(--gold)" />
    </g>

    <!-- Collines. -->
    <path d="M0 820 C 300 770 600 800 900 820 S 1400 780 1600 800 V1000 H0Z" style="fill: var(--hill-1)" />
    <path d="M0 900 C 400 860 800 900 1100 910 S 1500 880 1600 890 V1000 H0Z" style="fill: var(--hill-2)" />

    <!-- Grandes feuilles aux angles. -->
    <g style="fill: var(--leaf-1)">
      <use href="#sol-broad" transform="translate(-30 1000) rotate(-38) scale(1.5)" />
      <use href="#sol-broad" transform="translate(1630 1000) rotate(-142) scale(1.5)" />
    </g>
    <g style="fill: var(--leaf-2)">
      <use href="#sol-broad" transform="translate(30 1010) rotate(-12) scale(1.1)" />
      <use href="#sol-broad" transform="translate(1570 1010) rotate(-168) scale(1.1)" />
    </g>

    <!-- Fougères. -->
    {#each fronds as f, i (i)}
      <g
        class="sway"
        style="transform-origin: {f.origin[0]}px {f.origin[1]}px; animation-delay: {f.delay}s; fill: var(--leaf-{f.tone}); stroke: var(--leaf-{f.tone})"
      >
        <path d={f.d} fill="none" stroke-width="3" stroke-linecap="round" />
        {#each f.leaves as l, j (j)}
          <use href="#sol-leaf" transform="translate({l.x} {l.y}) rotate({l.a}) scale({l.s})" stroke="none" />
        {/each}
      </g>
    {/each}

    <!-- Vrilles qui descendent du haut. -->
    {#each vines as v, i (i)}
      <g
        class="sway slow"
        style="transform-origin: {v.origin[0]}px {v.origin[1]}px; animation-delay: {v.delay}s; fill: var(--leaf-2); stroke: var(--leaf-1)"
      >
        <path d={v.d} fill="none" stroke-width="2" stroke-linecap="round" />
        {#each v.leaves as l, j (j)}
          <use href="#sol-leaf" transform="translate({l.x} {l.y}) rotate({l.a}) scale({l.s})" stroke="none" />
        {/each}
      </g>
    {/each}

    <rect width="1600" height="1000" filter="url(#sol-grain)" opacity="0.12" />
  </svg>
</div>

<style>
  /* Mélanges en oklch : la teinte de l'album reste discrète, le vert domine. */
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    --sky-top: color-mix(in oklch, var(--album) 10%, #f9f0cf);
    --sky-low: color-mix(in oklch, var(--album) 6%, #e4edcd);
    --glow: #f2c14e;
    --gold: #d99a1e;
    --city: color-mix(in oklch, var(--album) 10%, #cdd9c0);
    --city-dark: color-mix(in oklch, var(--album) 8%, #9fb59a);
    --panel: #6f8fa3;
    --hill-1: color-mix(in oklch, var(--album) 8%, #c9dcae);
    --hill-2: color-mix(in oklch, var(--album) 6%, #a9c78f);
    --leaf-1: #2a5a35;
    --leaf-2: #3d7b47;
    --leaf-3: #6c9e57;
  }

  .intense {
    --sky-top: color-mix(in oklch, var(--album) 18%, #f9f0cf);
  }

  svg {
    width: 100%;
    height: 100%;
  }

  .city {
    opacity: 0.55;
  }

  /* « En lecture » : la ville passe derrière les commandes, elle s'efface davantage. */
  .intense .city {
    opacity: 0.3;
  }

  .blades {
    transform-origin: 250px 540px;
  }

  :global(:root[data-playing="true"]) .sway {
    animation: sway 7s ease-in-out infinite alternate;
  }

  :global(:root[data-playing="true"]) .sway.slow {
    animation-duration: 11s;
  }

  :global(:root[data-playing="true"]) .blades {
    animation: spin 14s linear infinite;
  }

  @keyframes sway {
    from {
      transform: rotate(-1.6deg);
    }
    to {
      transform: rotate(1.6deg);
    }
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
