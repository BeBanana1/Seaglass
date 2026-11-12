<script lang="ts">
  // Playlist intelligente locale (#383) : la liste est calculée par le backend (`smart_list`),
  // la page l'affiche comme un mix. « Ajouter à une playlist » l'enregistre chez TIDAL.
  import { commands, type PlaySource, type SmartList, type Track } from "../lib/ipc";
  import { formatLongDuration } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { platform } from "../lib/platform.svelte";

  let { list }: { list: SmartList } = $props();

  let tracks = $state<Track[] | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const asked = list;
    let stale = false;
    tracks = null;
    error = null;
    commands
      .smartList(asked)
      .then((found) => {
        if (!stale) tracks = found;
      })
      .catch((e) => {
        if (!stale) error = String(e);
      });
    return () => (stale = true);
  });

  const title = $derived(t.views.smart.names[list]);
  const source = $derived<PlaySource>({ kind: "smart", list });
  const duration = $derived((tracks ?? []).reduce((sum, track) => sum + track.duration, 0));
</script>

<section class="smart-list">
  <PageHeader
    image={tracks?.[0]?.album.coverLargeUrl ?? tracks?.[0]?.album.coverUrl ?? null}
    eyebrow={t.views.smart.eyebrow}
    {title}
    description={t.views.smart.rules[list]}
    trackIds={(tracks ?? []).map((track) => track.id)}
    {source}
    pin={{ kind: "smart", list }}
    queueable
  >
    {#snippet meta()}
      {#if tracks}
        {t.common.tracks(tracks.length)}
        {#if formatLongDuration(duration)} · {formatLongDuration(duration)}{/if}
      {/if}
    {/snippet}
  </PageHeader>
  <p class="smart-scope">{t.views.smart.scope}</p>
  {#if error}
    <p class="error">{error}</p>
  {:else if !tracks}
    <p class="muted">{t.common.loading}</p>
  {:else if tracks.length === 0}
    <p class="muted">{t.views.smart.empty}</p>
  {:else}
    <TrackList {tracks} {source} numbering={platform.mobile ? "none" : "index"} />
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .smart-scope {
    margin: -8px 0 16px;
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
