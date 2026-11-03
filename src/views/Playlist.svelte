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
  import { listSort, refreshPinTitle, saveListSort } from "../lib/stores/ui.svelte";
  import { notePlaylistChange, playlistChanges } from "../lib/stores/playlistPicker.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { listSearch } from "../lib/stores/listSearch.svelte";
  import { t } from "../lib/i18n.svelte";

  const SORT_KEYS: SortKey[] = ["position", "added", "title", "artist", "album", "duration"];
  const DEFAULT_SORT: TrackSort = { key: "position", descending: false };
  // « Chargement… » seulement si l'ouverture dure : rien ne clignote sur une page rapide (#152).
  const LOADING_DELAY_MS = 300;

  let { uuid }: { uuid: string } = $props();

  let page = $state<PlaylistPage | null>(null);
  let error = $state<string | null>(null);
  let slow = $state(false);
  let query = $state("");
  let sort = $state<TrackSort>(DEFAULT_SORT);

  $effect(() => {
    const target = uuid;
    page = null;
    error = null;
    slow = false;
    query = "";
    // Tri mémorisé pour cette playlist (#93) ; le filtre repart vide.
    sort = untrack(() => listSort(target, DEFAULT_SORT));
    const timer = setTimeout(() => (slow = true), LOADING_DELAY_MS);
    open(target);
    return () => clearTimeout(timer);
  });

  // Titres ajoutés ou retirés : rechargement sur place (filtre et tri gardés).
  $effect(() => {
    void playlistChanges.revision;
    if (playlistChanges.uuid === untrack(() => uuid)) untrack(() => load(uuid));
  });

  function setSort(next: TrackSort) {
    sort = next;
    saveListSort(uuid, next).catch((e) => pushToast({ level: "error", message: String(e) }));
  }

  // Copie gardée par le backend affichée aussitôt (#152), puis vérifiée : elle n'est
  // remplacée que si la playlist a changé depuis.
  async function open(target: string) {
    const cached = await commands.cachedPlaylist(target).catch(() => null);
    if (target !== uuid) return;
    if (cached && !page) page = cached;
    await load(target);
  }

  async function load(target: string) {
    try {
      const fresh = await commands.playlist(target, untrack(() => page?.revision ?? null));
      if (target !== uuid || !fresh) return;
      page = fresh;
      // Une playlist épinglée puis renommée : la barre latérale reprend le nouveau titre.
      refreshPinTitle(target, fresh.playlist.title).catch(() => {});
    } catch (e) {
      if (target !== uuid) return;
      // Copie gardée déjà affichée (hors ligne, par exemple) : elle reste, l'erreur part en toast.
      if (page) pushToast({ level: "error", message: String(e) });
      else error = String(e);
    }
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
    {#if slow}<p class="muted">{t.common.loading}</p>{/if}
  {:else}
    {@const playlist = page.playlist}
    <!-- Android : en-tête masqué pendant une recherche dans la playlist (#151). -->
    {#if !listSearch.active}
      <PageHeader
        image={playlist.imageUrl}
        eyebrow={t.format.playlist}
        title={playlist.title}
        description={playlist.description}
        trackIds={sorted.map((l) => l.track.id)}
        source={{ kind: "playlist", uuid: playlist.uuid, title: playlist.title }}
        queueable
        pin={{ kind: "playlist", uuid: playlist.uuid, title: playlist.title }}
        favorite={playlist.mine ? undefined : { kind: "playlist", uuid: playlist.uuid }}
      >
        {#snippet meta()}
          {playlistSubtitle(playlist)}
          {#if formatLongDuration(playlist.duration)} · {formatLongDuration(playlist.duration)}{/if}
        {/snippet}
      </PageHeader>
    {/if}
    {#if page.tracks.length > 1}
      <TrackTools
        bind:query
        bind:sort={() => sort, setSort}
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
        source={{ kind: "playlist", uuid: page.playlist.uuid, title: page.playlist.title }}
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
