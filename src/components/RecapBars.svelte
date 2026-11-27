<script lang="ts">
  // Histogram of the recap (#417): listens per day of the week or per hour.
  let {
    values,
    labels,
    titles,
    top,
    label,
  }: {
    values: number[];
    /** Marker under each bar (empty to keep only a few of them). */
    labels: string[];
    /** Tooltip of each bar. */
    titles: string[];
    /** Highlighted bar (the most listened). */
    top: number | null;
    label: string;
  } = $props();

  const max = $derived(Math.max(1, ...values));
</script>

<div class="recap-bars" role="img" aria-label={label}>
  {#each values as n, i (i)}
    <div class="recap-bar" class:top={i === top} title={titles[i]}>
      <i style:height={`${(n / max) * 100}%`}></i>
      <span>{labels[i]}</span>
    </div>
  {/each}
</div>

<style>
  .recap-bars {
    display: flex;
    align-items: flex-end;
    gap: 3px;
    height: 72px;
    margin-top: 8px;
  }

  .recap-bar {
    display: flex;
    flex: 1;
    flex-direction: column;
    justify-content: flex-end;
    align-items: center;
    height: 100%;
    gap: 3px;
  }

  .recap-bar i {
    width: 100%;
    min-height: 2px;
    border-radius: 3px 3px 0 0;
    background: color-mix(in srgb, var(--accent) 40%, transparent);
  }

  .recap-bar.top i {
    background: var(--accent);
  }

  .recap-bar span {
    height: 14px;
    font-size: 10px;
    color: var(--text-muted);
  }
</style>
