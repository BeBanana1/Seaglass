<script lang="ts">
  // Fond Cyberpunk (#66) : skyline sombre en contre-jour, fenêtres allumées, enseignes
  // verticales, brume néon à l'horizon, pluie et lignes de balayage. Le centre reste sombre
  // pour le contenu ; la pluie ne tombe que pendant la lecture (`data-playing` sur <html>).
  let { intense = false }: { intense?: boolean } = $props();

  // Générateur pseudo-aléatoire déterministe : la ville est la même à chaque lancement.
  let seed = 7;
  const rand = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;

  const ground = 1000;
  const colors = ["var(--cyan)", "var(--magenta)", "var(--yellow)"];

  function skyline(count: number, minH: number, maxH: number, x0: number, x1: number) {
    const towers = [];
    let x = x0;
    while (x < x1 && towers.length < count) {
      const w = 40 + rand() * 80;
      const h = minH + rand() * (maxH - minH);
      const windows = [];
      for (let wy = ground - h + 14; wy < ground - 20; wy += 16) {
        for (let wx = x + 8; wx < x + w - 10; wx += 12) {
          if (rand() < 0.16) windows.push({ x: wx, y: wy, c: colors[Math.floor(rand() * 3)] });
        }
      }
      towers.push({ x, w, h, windows, antenna: rand() < 0.3 });
      x += w + 4 + rand() * 18;
    }
    return towers;
  }

  const far = skyline(40, 220, 420, -20, 1620);
  const near = [...skyline(6, 380, 640, -30, 330), ...skyline(6, 380, 640, 1270, 1640)];

  const signs = [
    { x: 150, y: 470, h: 150, c: "var(--magenta)" },
    { x: 1400, y: 420, h: 180, c: "var(--cyan)" },
    { x: 1520, y: 560, h: 110, c: "var(--yellow)" },
  ];

  const drops = Array.from({ length: 70 }, () => ({
    x: rand() * 1700 - 50,
    y: rand() * 1000,
    l: 30 + rand() * 50,
    d: -Math.round(rand() * 120) / 100,
  }));
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMax slice">
    <defs>
      <linearGradient id="cy-sky" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" stop-color="#05060b" />
        <stop offset="0.55" stop-color="#0b0c1c" />
        <stop offset="1" stop-color="#1a0b22" />
      </linearGradient>
      <radialGradient id="cy-haze" cx="0.5" cy="1" r="0.75">
        <stop offset="0" stop-color="#ff2a6d" stop-opacity="0.38" />
        <stop offset="0.45" stop-color="#7a2bff" stop-opacity="0.16" />
        <stop offset="1" stop-color="#05060b" stop-opacity="0" />
      </radialGradient>
      <pattern id="cy-scan" width="4" height="4" patternUnits="userSpaceOnUse">
        <rect width="4" height="1" fill="#000" opacity="0.35" />
      </pattern>
    </defs>

    <rect width="1600" height="1000" fill="url(#cy-sky)" />
    <rect width="1600" height="1000" fill="url(#cy-haze)" />

    <!-- Arrière-plan : skyline lointaine, fenêtres pâles. -->
    <g class="far">
      {#each far as t, i (i)}
        <rect x={t.x} y={ground - t.h} width={t.w} height={t.h} fill="#0d0f1e" />
        {#each t.windows as w, j (j)}
          <rect x={w.x} y={w.y} width="5" height="7" style="fill: {w.c}" opacity="0.35" />
        {/each}
      {/each}
    </g>

    <!-- Premier plan sur les bords : tours plus hautes, antennes, enseignes. -->
    <g class="near">
      {#each near as t, i (i)}
        <rect x={t.x} y={ground - t.h} width={t.w} height={t.h} fill="#05060c" />
        <rect
          x={t.x}
          y={ground - t.h}
          width={t.w}
          height="2"
          style="fill: var(--cyan)"
          opacity="0.5"
        />
        {#if t.antenna}
          <path d="M{t.x + t.w / 2} {ground - t.h} v-60" stroke="#1b1e33" stroke-width="3" />
          <circle
            class="blink"
            cx={t.x + t.w / 2}
            cy={ground - t.h - 60}
            r="3"
            style="fill: var(--magenta)"
          />
        {/if}
        {#each t.windows as w, j (j)}
          <rect x={w.x} y={w.y} width="5" height="7" style="fill: {w.c}" opacity="0.75" />
        {/each}
      {/each}
      {#each signs as s, i (i)}
        <rect
          class="sign"
          x={s.x}
          y={s.y}
          width="22"
          height={s.h}
          fill="none"
          stroke-width="3"
          style="stroke: {s.c}; filter: drop-shadow(0 0 8px {s.c})"
        />
        {#each Array.from({ length: Math.floor(s.h / 26) }) as _, k (k)}
          <rect
            x={s.x + 7}
            y={s.y + 10 + k * 26}
            width="8"
            height="14"
            style="fill: {s.c}"
            opacity="0.8"
          />
        {/each}
      {/each}
    </g>

    <!-- Pluie. -->
    <g class="rain" stroke="#8fd6ff" stroke-width="1.2" opacity="0.28">
      {#each drops as d, i (i)}
        <line
          x1={d.x}
          y1={d.y}
          x2={d.x - d.l * 0.25}
          y2={d.y + d.l}
          style="animation-delay: {d.d}s"
        />
      {/each}
    </g>

    <rect width="1600" height="1000" fill="url(#cy-scan)" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    --cyan: #00e5ff;
    --magenta: #ff2a6d;
    --yellow: #fcee0a;
  }

  svg {
    width: 100%;
    height: 100%;
  }

  /* « En lecture » : la ville recule, le contenu passe devant. */
  .intense .far {
    opacity: 0.6;
  }

  .intense .near {
    opacity: 0.7;
  }

  :global(:root[data-playing="true"]) .rain line {
    animation: rain 1.2s linear infinite;
  }

  :global(:root[data-playing="true"]) .blink {
    animation: blink 2.4s steps(1) infinite;
  }

  @keyframes rain {
    from {
      transform: translate(0, -160px);
    }
    to {
      transform: translate(-40px, 160px);
    }
  }

  @keyframes blink {
    50% {
      opacity: 0.15;
    }
  }
</style>
