<script lang="ts">
  import type { MediaItem } from "../lib/ipc";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import { replace, type LibraryTab } from "../lib/stores/nav.svelte";
  import {
    filterTracks,
    listTracks,
    sortTracks,
    type SortKey,
    type TrackSort,
  } from "../lib/trackOrder";
  import CardGrid from "../components/CardGrid.svelte";
  import TrackList from "../components/TrackList.svelte";
  import TrackTools from "../components/TrackTools.svelte";
  import PinButton from "../components/PinButton.svelte";

  // Titres likés : l'API les renvoie du plus récent au plus ancien (date du like).
  const TRACK_SORT_KEYS: SortKey[] = ["added", "title", "artist", "album", "duration"];

  let { tab }: { tab: LibraryTab } = $props();

  let query = $state("");
  let sort = $state<TrackSort>({ key: "added", descending: true });

  const allTracks = $derived(library.tracks.items ?? []);
  // Le contexte de lecture est la liste affichée, dans l'ordre affiché.
  const shownTracks = $derived(filterTracks(sortTracks(listTracks(allTracks), sort), query));

  const tabs: { id: LibraryTab; label: string }[] = [
    { id: "playlists", label: "Playlists" },
    { id: "mixes", label: "Mixes" },
    { id: "albums", label: "Albums" },
    { id: "artists", label: "Artistes" },
    { id: "tracks", label: "Titres" },
  ];

  $effect(() => {
    loadLibrary(tab);
  });

  const slot = $derived(library[tab]);

  const cards = $derived.by((): MediaItem[] => {
    switch (tab) {
      case "playlists":
        return (library.playlists.items ?? []).map((p) => ({ kind: "playlist", ...p }));
      case "mixes":
        return (library.mixes.items ?? []).map((m) => ({ kind: "mix", ...m }));
      case "albums":
        return (library.albums.items ?? []).map((a) => ({ kind: "album", ...a }));
      case "artists":
        return (library.artists.items ?? []).map((a) => ({ kind: "artist", ...a }));
      default:
        return [];
    }
  });
</script>

<section>
  <div class="top">
    <h1>Bibliothèque</h1>
    <div class="top-actions">
      {#if tab === "tracks"}<PinButton pin={{ kind: "likedTracks" }} />{/if}
      <button class="refresh" onclick={() => loadLibrary(tab, true)} disabled={slot.loading}>
        Actualiser
      </button>
    </div>
  </div>
  <div class="tabs" role="tablist">
    {#each tabs as t (t.id)}
      <button
        role="tab"
        aria-selected={t.id === tab}
        class:active={t.id === tab}
        onclick={() => replace({ name: "library", tab: t.id })}
      >
        {t.label}
        {#if library[t.id].items}<span class="count">{library[t.id].items?.length}</span>{/if}
      </button>
    {/each}
  </div>

  {#if slot.error}
    <p class="error">{slot.error}</p>
  {:else if !slot.items}
    <p class="muted">Chargement…</p>
  {:else if slot.items.length === 0}
    <p class="muted">Rien ici pour l'instant.</p>
  {:else if tab === "tracks"}
    <TrackTools
      bind:query
      bind:sort
      keys={TRACK_SORT_KEYS}
      shown={shownTracks.length}
      total={allTracks.length}
    />
    {#if shownTracks.length === 0}
      <p class="muted">Aucun titre ne correspond à « {query.trim()} ».</p>
    {:else}
      <TrackList tracks={shownTracks.map((t) => t.track)} />
    {/if}
  {:else}
    <CardGrid items={cards} />
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .top-actions {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  h1 {
    margin: 0 10px;
    font-size: 26px;
  }

  .refresh {
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
  }

  .refresh:hover:not(:disabled) {
    color: var(--text);
  }

  .tabs {
    display: flex;
    gap: 6px;
    margin: 16px 10px 20px;
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

  .count {
    margin-left: 4px;
    font-size: 11px;
    opacity: 0.7;
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }
</style>
