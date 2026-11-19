<script lang="ts">
  // Recap heatmap (#521): plays by weekday (rows, Monday first) and local hour (columns),
  // hand-made SVG. The cell opacity follows its share of the busiest cell.
  let {
    grid,
    days,
    dayNames,
    label,
    title,
  }: {
    /** 7 rows of 24 counts. */
    grid: number[][];
    /** Short label of each row. */
    days: string[];
    /** Full name of each row, for the tooltips. */
    dayNames: string[];
    label: string;
    /** Tooltip of a cell. */
    title: (day: string, hour: number, plays: number) => string;
  } = $props();

  const CELL = 12;
  const GAP = 2;
  const LEFT = 14;
  const BOTTOM = 14;
  const width = LEFT + 24 * (CELL + GAP);
  const height = 7 * (CELL + GAP) + BOTTOM;

  const max = $derived(Math.max(1, ...grid.flat()));
</script>

<svg class="recap-heatmap" viewBox={`0 0 ${width} ${height}`} role="img" aria-label={label}>
  {#each grid as row, d (d)}
    <text class="recap-heatmap-day" x="0" y={d * (CELL + GAP) + CELL - 2}>{days[d]}</text>
    {#each row as n, h (h)}
      <rect
        x={LEFT + h * (CELL + GAP)}
        y={d * (CELL + GAP)}
        width={CELL}
        height={CELL}
        rx="2"
        class:empty={n === 0}
        style:fill-opacity={n === 0 ? undefined : 0.15 + (0.85 * n) / max}
      >
        <title>{title(dayNames[d], h, n)}</title>
      </rect>
    {/each}
  {/each}
  {#each [0, 6, 12, 18] as h (h)}
    <text class="recap-heatmap-hour" x={LEFT + h * (CELL + GAP)} y={height - 2}>{h}</text>
  {/each}
</svg>

<style>
  .recap-heatmap {
    width: 100%;
    height: auto;
    margin-top: 8px;
  }

  rect {
    fill: var(--accent);
  }

  rect.empty {
    fill: var(--text-muted);
    fill-opacity: 0.08;
  }

  text {
    font-size: 9px;
    fill: var(--text-muted);
  }
</style>
