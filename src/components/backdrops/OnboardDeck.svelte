<script lang="ts">
  // Fond Ordinateur de bord (#339) : console de vaisseau dessinée avant l'image de synthèse.
  // Grille fine, repères d'angle, cadrans ronds à droite, blocs de couleur numérotés et
  // relevés chiffrés en bas à gauche, bande d'avertissement. Rien n'est animé.
  let { intense = false }: { intense?: boolean } = $props();

  // Relevés fixes (décor : aucune mesure réelle), générés une fois avec une graine.
  let seed = 7;
  const rand = () => ((seed = (seed * 16807) % 2147483647) - 1) / 2147483646;
  const readouts = Array.from({ length: 6 }, () =>
    Array.from({ length: 4 }, () => Math.floor(rand() * 10000).toString().padStart(4, "0")).join(" "),
  );
  const blocks = ["var(--ob-red)", "var(--ob-orange)", "var(--ob-yellow)", "var(--ob-teal)"];
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <pattern id="ob-grid" width="40" height="40" patternUnits="userSpaceOnUse">
        <path d="M40 0 H0 V40" fill="none" stroke="var(--text)" stroke-width="0.6" opacity="0.07" />
      </pattern>
    </defs>
    <rect width="1600" height="1000" fill="var(--bg)" />
    <rect width="1600" height="1000" fill="url(#ob-grid)" />

    <!-- Repères d'angle. -->
    <g class="brackets" fill="none" stroke="var(--text)" stroke-width="2">
      <path d="M24 84 V24 H84" />
      <path d="M1516 24 H1576 V84" />
      <path d="M24 916 V976 H84" />
      <path d="M1516 976 H1576 V916" />
    </g>

    <!-- Cadrans : anneaux gradués, aiguille fixe. -->
    <g class="dials" fill="none" stroke="var(--text)">
      {#each [[1440, 300, 90], [1470, 560, 60]] as [cx, cy, r], i (i)}
        <circle {cx} {cy} {r} stroke-width="1.5" opacity="0.35" />
        <circle {cx} {cy} r={r - 14} stroke-width="8" stroke-dasharray="2 9" opacity="0.25" />
        <path
          d="M{cx} {cy} L{cx + (r - 20) * Math.cos(-0.7 - i)} {cy + (r - 20) * Math.sin(-0.7 - i)}"
          stroke="var(--ob-orange)"
          stroke-width="3"
        />
        <circle {cx} {cy} r="5" fill="var(--ob-red)" stroke="none" />
      {/each}
    </g>

    <!-- Blocs de couleur numérotés et relevés. -->
    <g class="panel-blocks">
      {#each blocks as fill, i (i)}
        <rect x={60 + i * 64} y="800" width="56" height="34" style:fill />
        <text x={66 + i * 64} y="826" class="num">{String(i + 1).padStart(2, "0")}</text>
      {/each}
      {#each readouts as line, i (i)}
        <text x="60" y={862 + i * 18} class="readout">{line}</text>
      {/each}
    </g>

    <!-- Bande d'avertissement en bas à droite. -->
    <rect class="hazard" x="1100" y="940" width="440" height="16" />
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

  .brackets {
    opacity: 0.5;
  }

  .panel-blocks {
    opacity: 0.75;
  }

  .num {
    font: 700 20px var(--font-mono);
    fill: #0b0c0b;
  }

  .readout {
    font: 13px var(--font-mono);
    letter-spacing: 0.2em;
    fill: var(--text-muted);
    opacity: 0.6;
  }

  .hazard {
    fill: var(--ob-yellow);
    opacity: 0.5;
  }

  /* « En lecture » : la bande passait sous la file, retirée ; cadrans et relevés plus sourds. */
  .intense .hazard {
    display: none;
  }

  .intense :is(.dials, .panel-blocks) {
    opacity: 0.25;
  }
</style>
