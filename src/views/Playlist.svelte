<script lang="ts">
  import { untrack } from "svelte";
  import { commands, type PlaylistPage } from "../lib/ipc";
  import { formatLongDuration, fullTitle, playlistSubtitle } from "../lib/format";
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
  import { notePlaylistChange, playlistChanges } from "../lib/stores/playlistPicker.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

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
    load(target);
  });

  // Titres ajoutés ou retirés : rechargement sur place (filtre et tri gardés).
  $effect(() => {
    void playlistChanges.revision;
    if (playlistChanges.uuid === untrack(() => uuid)) untrack(() => load(uuid));
  });

  function load(target: string) {
    commands
      .playlist(target)
      .then((p) => {
        if (target !== uuid) return;
        page = p;
        // Une playlist épinglée puis renommée : la barre latérale reprend le nouveau titre.
        refreshPinTitle(target, p.playlist.title).catch(() => {});
      })
      .catch((e) => (error = String(e)));
  }

  // `index` : position dans la liste affichée ; le backend reçoit la position d'origine.
  async function removeAt(index: number) {
    const entry = shown[index];
    if (!entry) return;
    try {
      await commands.removeFromPlaylist(uuid, entry.position - 1, entry.track.id);
      pushToast({ level: "info", message: t.views.playlist.removed(fullTitle(entry.track)) });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    // Rechargée même en cas d'échec : un refus signifie que la playlist a changé.
    notePlaylistChange(uuid);
  }

  // Lecture / Aléatoire suivent le tri ; un clic sur un titre met en contexte la liste
  // affichée (triée et filtrée), dans cet ordre.
  const sorted = $derived(page ? sortTracks(listTracks(page.tracks), sort) : []);
  const shown = $derived(filterTracks(sorted, query));
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    <p class="muted">{t.common.loading}</p>
  {:else}
    {@const playlist = page.playlist}
    <PageHeader
      image={playlist.imageUrl}
      eyebrow={t.format.playlist}
      title={playlist.title}
      description={playlist.description}
      trackIds={sorted.map((l) => l.track.id)}
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
      <p class="muted empty">{t.library.noMatch(query.trim())}</p>
    {:else}
      <TrackList
        tracks={shown.map((l) => l.track)}
        positions={shown.map((l) => l.position)}
        numbering="index"
        onRemoveFromPlaylist={page.playlist.mine ? removeAt : undefined}
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
