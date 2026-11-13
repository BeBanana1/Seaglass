<script lang="ts">
  // Histogramme de la rétrospective (#417) : écoutes par jour de la semaine ou par heure.
  let {
    values,
    labels,
    titles,
    top,
    label,
  }: {
    values: number[];
    /** Repère sous chaque barre (vide pour n'en garder que quelques-uns). */
    labels: string[];
    /** Info-bulle de chaque barre. */
    titles: string[];
    /** Barre mise en avant (la plus écoutée). */
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
