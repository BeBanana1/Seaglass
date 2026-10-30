<script lang="ts">
  import { commands, type PlaylistPage } from "../lib/ipc";
  import { formatLongDuration, playlistSubtitle } from "../lib/format";
  import {
    filterTracks,
    listTracks,
    sortTracks,
    type SortKey,
    type TrackSort,
  } from "../lib/trackOrder";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import TrackTools from "../components/TrackTools.svelte";
  import { refreshPinTitle } from "../lib/stores/ui.svelte";

  const SORT_KEYS: SortKey[] = ["position", "added", "title", "artist", "album", "duration"];
  const DEFAULT_SORT: TrackSort = { key: "position", descending: false };

  let { uuid }: { uuid: string } = $props();

  let page = $state<PlaylistPage | null>(null);
  let error = $state<string | null>(null);
  let query = $state("");
  let sort = $state<TrackSort>(DEFAULT_SORT);

  $effect(() => {
    const target = uuid;
    page = null;
    error = null;
    query = "";
    sort = DEFAULT_SORT;
    commands
      .playlist(target)
      .then((p) => {
        if (target !== uuid) return;
        page = p;
        // Une playlist épinglée puis renommée : la barre latérale reprend le nouveau titre.
        refreshPinTitle(target, p.playlist.title).catch(() => {});
      })
      .catch((e) => (error = String(e)));
  });

  // Lecture / Aléatoire suivent le tri ; un clic sur un titre met en contexte la liste
  // affichée (triée et filtrée), dans cet ordre.
  const sorted = $derived(page ? sortTracks(listTracks(page.tracks), sort) : []);
  const shown = $derived(filterTracks(sorted, query));
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    <p class="muted">Chargement…</p>
  {:else}
    {@const playlist = page.playlist}
    <PageHeader
      image={playlist.imageUrl}
      eyebrow="Playlist"
      title={playlist.title}
      description={playlist.description}
      trackIds={sorted.map((t) => t.track.id)}
      queueable
      pin={{ kind: "playlist", uuid: playlist.uuid, title: playlist.title }}
    >
      {#snippet meta()}
        {playlistSubtitle(playlist)}
        {#if formatLongDuration(playlist.duration)} · {formatLongDuration(playlist.duration)}{/if}
      {/snippet}
    </PageHeader>
    {#if page.tracks.length > 1}
      <TrackTools
        bind:query
        bind:sort
        keys={SORT_KEYS}
        shown={shown.length}
        total={page.tracks.length}
      />
    {/if}
    {#if shown.length === 0 && query.trim()}
      <p class="muted empty">Aucun titre ne correspond à « {query.trim()} ».</p>
    {:else}
      <TrackList
        tracks={shown.map((t) => t.track)}
        positions={shown.map((t) => t.position)}
        numbering="index"
      />
    {/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .muted {
    color: var(--text-muted);
  }

  .empty {
    margin: 0 10px;
  }

  .error {
    color: var(--danger);
  }
</style>
