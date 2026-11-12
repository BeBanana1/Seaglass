<script lang="ts">
  // Statistiques d'écoute locales (#326) : tout est calculé par le backend (`play_stats`),
  // la page n'affiche que le résultat de la période choisie.
  import { commands, SMART_LISTS, type MediaItem, type PlayStats, type StatsPeriod } from "../lib/ipc";
  import { go } from "../lib/stores/nav.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import TrackList from "../components/TrackList.svelte";
  import Icon from "../components/Icon.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t, num } from "../lib/i18n.svelte";

  const PERIODS: StatsPeriod[] = ["week", "month", "year", "all"];

  let period = $state<StatsPeriod>("month");
  let stats = $state<PlayStats | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const asked = period;
    let stale = false;
    error = null;
    commands
      .playStats(asked)
      .then((s) => {
        if (!stale) stats = s;
      })
      .catch((e) => {
        if (!stale) error = String(e);
      });
    return () => (stale = true);
  });

  const tracks = $derived(stats?.tracks.map((c) => c.item) ?? []);
  const trackNotes = $derived(stats?.tracks.map((c) => t.library.stats.plays(c.plays)) ?? []);
  const albums = $derived<MediaItem[]>(stats?.albums.map((c) => ({ kind: "album", ...c.item })) ?? []);
  const artists = $derived<MediaItem[]>(stats?.artists.map((c) => ({ kind: "artist", ...c.item })) ?? []);

  const time = $derived.by(() => {
    const minutes = Math.round((stats?.seconds ?? 0) / 60);
    return t.library.stats.time(Math.floor(minutes / 60), minutes % 60);
  });

  // « Lire » : le classement des titres de la période, dans l'ordre.
  async function playTop(shuffle: boolean) {
    try {
      await commands.playTracks(
        tracks.map((track) => track.id),
        0,
        shuffle,
        null,
      );
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<section class="stats">
  <h1>{t.library.stats.title}</h1>
  <!-- Playlists intelligentes locales (#383), calculées sur le même historique. -->
  <h2 class="smart-title">{t.views.smart.title}</h2>
  <div class="smart-lists">
    {#each SMART_LISTS as list (list)}
      <button class="smart-link" onclick={() => go({ name: "smart", list })}>
        <span class="smart-name">{t.views.smart.names[list]}</span>
        <span class="smart-rule">{t.views.smart.rules[list]}</span>
      </button>
    {/each}
  </div>
  <div class="tabs" role="tablist">
    {#each PERIODS as id (id)}
      <button role="tab" aria-selected={id === period} class:active={id === period} onclick={() => (period = id)}>
        {t.library.stats.periods[id]}
      </button>
    {/each}
  </div>

  {#if error}
    <p class="error">{error}</p>
  {:else if stats}
    {#if stats.plays === 0}
      <p class="muted">{t.library.stats.empty}</p>
    {:else}
      <div class="totals">
        <div class="total">
          <span class="value">{num(stats.plays)}</span>
          <span class="label">{t.library.stats.playsLabel}</span>
        </div>
        <div class="total">
          <span class="value">{time}</span>
          <span class="label">{t.library.stats.timeLabel}</span>
        </div>
      </div>
      {#if stats.unknown > 0}<p class="hint">{t.library.stats.unknown(stats.unknown)}</p>{/if}

      {#if tracks.length > 0}
        <div class="section-head">
          <h2>{t.library.stats.tracks}</h2>
          <!-- Mêmes classes que l'en-tête d'une page : les thèmes habillent ces boutons. -->
          <div class="actions">
            <button class="primary" onclick={() => playTop(false)}>
              <Icon name="play" size={16} /> {t.common.play}
            </button>
            <button class="secondary" onclick={() => playTop(true)}>
              <Icon name="shuffle" size={16} /> {t.common.shuffle}
            </button>
          </div>
        </div>
        <TrackList {tracks} numbering="index" notes={trackNotes} />
      {/if}
      <div class="cards">
        <CardGrid
          title={t.library.stats.albums}
          items={albums}
          row
          notes={stats.albums.map((c) => t.library.stats.plays(c.plays))}
        />
        <CardGrid
          title={t.library.stats.artists}
          items={artists}
          row
          notes={stats.artists.map((c) => t.library.stats.plays(c.plays))}
        />
      </div>
    {/if}
    <p class="hint">{t.library.stats.note}</p>
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  h1 {
    margin: 0 10px;
    font-size: 26px;
  }

  .tabs {
    display: flex;
    gap: 6px;
    margin: 16px 10px 20px;
  }

  .smart-title {
    margin: 20px 10px 10px;
  }

  .smart-lists {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 10px;
    margin: 0 10px 8px;
  }

  .smart-link {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius, 10px);
    background: var(--bg-raised);
    text-align: left;
  }

  .smart-link:hover {
    background: var(--bg-hover);
  }

  .smart-name {
    font-weight: 600;
  }

  .smart-rule {
    font-size: 12px;
    color: var(--text-muted);
  }

  .tabs button {
    padding: 6px 14px;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .tabs button.active {
    color: var(--bg);
    background: var(--text);
  }

  .totals {
    display: flex;
    flex-wrap: wrap;
    gap: 12px;
    margin: 0 10px 8px;
  }

  .total {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 140px;
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius, 10px);
    background: var(--bg-raised);
  }

  .total .value {
    font-size: 22px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .total .label {
    font-size: 12px;
    color: var(--text-muted);
  }

  .section-head {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    margin: 24px 10px 10px;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  .actions {
    display: flex;
    gap: 10px;
  }

  .actions button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    border-radius: var(--radius-pill);
    font-weight: 600;
    clip-path: var(--control-clip);
  }

  .primary {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:hover {
    background: var(--accent-strong);
  }

  .secondary {
    border: 1px solid var(--border) !important;
  }

  .secondary:hover {
    background: var(--bg-hover);
  }

  .cards {
    margin-top: 28px;
  }

  .muted,
  .hint {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .hint {
    margin-top: 8px;
    font-size: 13px;
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }

  :global(:root[data-platform="android"]) .tabs {
    overflow-x: auto;
    scrollbar-width: none;
  }

  :global(:root[data-platform="android"]) .tabs button {
    flex-shrink: 0;
    white-space: nowrap;
  }
</style>
