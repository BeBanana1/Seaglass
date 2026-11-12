<script lang="ts">
  // Fond Estampe (#338) : gravure japonaise en aplats. Ciel en dégradé d'indigo (bokashi) en
  // haut, soleil vermillon et bandes de nuages à droite, motif de vagues concentriques
  // (seigaiha) dans le coin bas gauche, horizon et vagues à crêtes en bas à droite. Le centre
  // reste au papier, pour le contenu. Rien n'est animé : une estampe ne bouge pas.
  let { intense = false }: { intense?: boolean } = $props();

  // Seigaiha : rangées d'écailles de vagues (cercles concentriques), chaque rangée posée sur
  // la précédente et décalée d'une demi-écaille. Dessinées dans l'ordre, sans `pattern`, pour
  // que le recouvrement soit net.
  const R = 44;
  const scales: { x: number; y: number }[] = [];
  for (let row = 0; row < 12; row++) {
    const y = 720 + row * (R / 2);
    const shift = row % 2 === 0 ? 0 : R;
    for (let x = -R + shift; x < 720; x += 2 * R) scales.push({ x, y });
  }
  const rings = [1, 0.75, 0.5, 0.25];

  // Vagues à crêtes : trois bandes de plus en plus proches, chacune une suite de creux et de
  // crêtes qui se recourbent.
  function wave(y: number, amp: number, step: number, from: number): string {
    let d = `M${from} 1000 L${from} ${y}`;
    for (let x = from; x < 1600 + step; x += step) {
      d += ` Q${x + step * 0.35} ${y - amp} ${x + step * 0.6} ${y - amp * 0.55}`;
      d += ` Q${x + step * 0.72} ${y - amp * 0.2} ${x + step * 0.62} ${y + amp * 0.05}`;
      d += ` Q${x + step * 0.85} ${y + amp * 0.15} ${x + step} ${y}`;
    }
    return `${d} L1600 1000 Z`;
  }
  const waves = [
    { d: wave(830, 46, 150, 940), cls: "far" },
    { d: wave(880, 60, 190, 900), cls: "mid" },
    { d: wave(940, 80, 240, 860), cls: "near" },
  ];
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <linearGradient id="es-sky" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" style="stop-color: var(--es-indigo)" stop-opacity="0.55" />
        <stop offset="1" style="stop-color: var(--es-indigo)" stop-opacity="0" />
      </linearGradient>
      <radialGradient id="es-fade" cx="0" cy="1" r="1">
        <stop offset="0.35" stop-color="white" />
        <stop offset="0.95" stop-color="black" />
      </radialGradient>
      <mask id="es-corner">
        <rect x="0" y="600" width="760" height="400" fill="url(#es-fade)" />
      </mask>
      <filter id="es-grain">
        <feTurbulence type="fractalNoise" baseFrequency="0.75" numOctaves="3" stitchTiles="stitch" />
        <feColorMatrix values="0 0 0 0 0.12  0 0 0 0 0.16  0 0 0 0 0.22  0 0 0 0.5 0" />
      </filter>
      <!-- Fibres du washi : traits longs et pâles. -->
      <filter id="es-fibres">
        <feTurbulence type="fractalNoise" baseFrequency="0.004 0.18" numOctaves="2" seed="7" />
        <feColorMatrix values="0 0 0 0 0.45  0 0 0 0 0.38  0 0 0 0 0.25  0 0 0 0.9 -0.35" />
      </filter>
    </defs>

    <!-- Ciel : bande d'indigo dégradée en haut (bokashi). -->
    <rect width="1600" height="150" fill="url(#es-sky)" />

    <!-- Soleil vermillon et bandes de nuages qui le traversent. -->
    <g class="sun">
      <circle cx="1400" cy="210" r="64" style="fill: var(--accent)" />
      <g class="clouds">
        <rect x="1250" y="226" width="260" height="10" rx="5" />
        <rect x="1300" y="246" width="300" height="10" rx="5" />
        <rect x="1180" y="176" width="190" height="8" rx="4" />
      </g>
    </g>

    <!-- Seigaiha dans le coin bas gauche, fondu vers le papier. -->
    <g class="seigaiha" mask="url(#es-corner)">
      {#each scales as s, i (i)}
        {#each rings as k, j (j)}
          <circle cx={s.x} cy={s.y} r={R * k} class:core={j === 3} />
        {/each}
      {/each}
    </g>

    <!-- Horizon et vagues à crêtes en bas à droite. -->
    <line class="horizon" x1="880" y1="790" x2="1600" y2="790" />
    <g class="waves">
      {#each waves as w (w.cls)}
        <path d={w.d} class={w.cls} />
      {/each}
    </g>

    <rect width="1600" height="1000" filter="url(#es-fibres)" opacity="0.2" />
    <rect width="1600" height="1000" filter="url(#es-grain)" opacity="0.14" />
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

  .clouds rect {
    fill: var(--bg);
  }

  .seigaiha circle {
    fill: var(--bg);
    stroke: var(--es-prussian);
    stroke-width: 2.4;
    opacity: 0.55;
  }

  .seigaiha circle.core {
    fill: color-mix(in srgb, var(--es-prussian) 35%, var(--bg));
  }

  .horizon {
    stroke: var(--es-indigo);
    stroke-width: 1.5;
    opacity: 0.35;
  }

  .waves path {
    stroke: var(--es-paper);
    stroke-width: 3;
    stroke-linejoin: round;
  }

  .waves .far {
    fill: color-mix(in srgb, var(--es-prussian) 35%, var(--bg));
  }

  .waves .mid {
    fill: color-mix(in srgb, var(--es-prussian) 65%, var(--bg));
  }

  .waves .near {
    fill: var(--es-indigo);
  }

  /* « En lecture » : le soleil tombait sous le titre et les vagues sous la file, à droite ;
     il ne reste que le ciel et le seigaiha, plus pâle, sous la pochette. */
  .intense :is(.sun, .waves, .horizon) {
    display: none;
  }

  .intense .seigaiha {
    opacity: 0.5;
  }
</style>
