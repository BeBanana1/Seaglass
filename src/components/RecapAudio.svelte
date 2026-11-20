<script lang="ts">
  // Audio side of the recap (#524): qualities, bit-perfect share and formats played most,
  // over the listens recorded with their format. Counts come from the backend (`recap`), the
  // page only shows them as percentages.
  import type { AudioRecap } from "../lib/ipc";
  import { formatStream } from "../lib/format";
  import { t, locale } from "../lib/i18n.svelte";

  let { audio, plays }: { audio: AudioRecap; plays: number } = $props();

  const percent = $derived(new Intl.NumberFormat(locale(), { style: "percent" }));
  const share = (n: number) => percent.format(n / audio.known);

  const qualities = $derived(
    (["hires", "lossless", "lossy"] as const)
      .map((id) => ({ id, n: audio[id] }))
      .filter((q) => q.n > 0),
  );

  const time = $derived.by(() => {
    const minutes = Math.round(audio.bitPerfectSeconds / 60);
    return t.library.stats.time(Math.floor(minutes / 60), minutes % 60);
  });
</script>

<div class="recap-quality" role="img" aria-label={t.library.recap.audioLabel}>
  {#each qualities as q (q.id)}
    <i class={q.id} style:flex-grow={q.n}></i>
  {/each}
</div>
<ul class="recap-quality-legend">
  {#each qualities as q (q.id)}
    <li>
      <span class={`dot ${q.id}`}></span>{t.library.recap.qualities[q.id]}
      <b>{share(q.n)}</b>
    </li>
  {/each}
</ul>
{#if audio.bitPerfect > 0}
  <p class="recap-audio-line">{t.library.recap.bitPerfect(share(audio.bitPerfect), time)}</p>
{/if}
{#if audio.formats.length > 0}
  <h3>{t.library.recap.formats}</h3>
  <ol class="recap-formats">
    {#each audio.formats as f, i (i)}
      <li><span>{formatStream(f)}</span><span>{t.library.stats.plays(f.plays)}</span></li>
    {/each}
  </ol>
{/if}
{#if audio.known < plays}
  <p class="recap-audio-note">{t.library.recap.audioNote(audio.known, plays)}</p>
{/if}

<style>
  .recap-quality {
    display: flex;
    gap: 2px;
    height: 12px;
    margin-top: 6px;
    overflow: hidden;
    border-radius: 6px;
  }

  .recap-quality i {
    flex-basis: 0;
    min-width: 3px;
  }

  .hires {
    background: var(--accent);
  }

  .lossless {
    background: color-mix(in srgb, var(--accent) 55%, transparent);
  }

  .lossy {
    background: color-mix(in srgb, var(--text-muted) 40%, transparent);
  }

  .recap-quality-legend {
    display: flex;
    flex-wrap: wrap;
    gap: 4px 14px;
    margin: 6px 0 0;
    padding: 0;
    list-style: none;
    font-size: 13px;
    color: var(--text-muted);
  }

  .recap-quality-legend b {
    margin-left: 4px;
    color: var(--text);
    font-weight: 600;
  }

  .dot {
    display: inline-block;
    width: 8px;
    height: 8px;
    margin-right: 5px;
    border-radius: 50%;
  }

  .recap-audio-line {
    margin: 6px 0 0;
  }

  h3 {
    margin: 10px 0 2px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .recap-formats {
    margin: 0;
    padding: 0;
    list-style: none;
    font-size: 14px;
  }

  .recap-formats li {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    padding: 2px 0;
    font-variant-numeric: tabular-nums;
  }

  .recap-formats li span:last-child {
    color: var(--text-muted);
  }

  .recap-audio-note {
    margin: 8px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
