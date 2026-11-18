<script lang="ts">
  // Fond Art déco (#338) : soleil levant en rayons dorés au bas de l'écran, arcs en gradins
  // autour de lui, gradins (ziggourats) dans les angles du haut, frise de chevrons en bas.
  // Tout est symétrique et fixe : rien n'est animé.
  let { intense = false }: { intense?: boolean } = $props();

  const CX = 800;
  const CY = 1060;

  // Rayons du soleil, de 196° à 344°, alternés long / court.
  const rays: { x: number; y: number }[] = [];
  for (let i = 0, a = 196; a <= 344; a += 4, i++) {
    const r = i % 2 === 0 ? 1100 : 780;
    const rad = (a * Math.PI) / 180;
    rays.push({ x: CX + r * Math.cos(rad), y: CY + r * Math.sin(rad) });
  }

  const arcs = [240, 280, 320, 420];

  // Gradins d'angle : L emboîtés, de plus en plus petits.
  const steps = [0, 1, 2, 3, 4].map((i) => 40 + i * 34);

  // Frise de chevrons sur toute la largeur.
  const chevrons: number[] = [];
  for (let x = 0; x <= 1600; x += 40) chevrons.push(x);
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <radialGradient id="deco-glow" cx="0.5" cy="1.06" r="0.7">
        <stop offset="0" stop-color="#1c2a44" />
        <stop offset="1" stop-color="#0a111e" />
      </radialGradient>
    </defs>

    <rect width="1600" height="1000" fill="url(#deco-glow)" />

    <g class="sun">
      {#each rays as r, i (i)}
        <line x1={CX} y1={CY} x2={r.x} y2={r.y} />
      {/each}
      <circle class="disc" cx={CX} cy={CY} r="200" />
      {#each arcs as r (r)}
        <circle class="arc" cx={CX} cy={CY} {r} />
      {/each}
    </g>

    <g class="steps">
      {#each steps as s (s)}
        <path d="M0 {s} H{s} V0" />
        <path d="M1600 {s} H{1600 - s} V0" />
      {/each}
    </g>

    <g class="frieze">
      <line x1="0" y1="958" x2="1600" y2="958" />
      <path d={chevrons.map((x) => `M${x} 990 L${x + 20} 970 L${x + 40} 990`).join(" ")} />
      <line x1="0" y1="996" x2="1600" y2="996" />
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

  .sun line {
    stroke: var(--ad-gold);
    stroke-width: 1.2;
    opacity: 0.16;
  }

  .sun .disc {
    fill: var(--bg);
    stroke: var(--ad-gold);
    stroke-width: 1.5;
    opacity: 0.9;
  }

  .sun .arc {
    fill: none;
    stroke: var(--ad-gold);
    stroke-width: 1;
    opacity: 0.3;
  }

  .steps path {
    fill: none;
    stroke: var(--ad-gold);
    stroke-width: 1.2;
    opacity: 0.35;
  }

  .frieze :is(line, path) {
    fill: none;
    stroke: var(--ad-gold);
    stroke-width: 1.2;
    opacity: 0.4;
  }

  /* « En lecture » : le soleil passe derrière les commandes, plus discret. */
  .intense .sun {
    opacity: 0.55;
  }
</style>
