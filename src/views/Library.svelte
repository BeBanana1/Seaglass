<script lang="ts">
  import type { MediaItem } from "../lib/ipc";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import { replace, type LibraryTab } from "../lib/stores/nav.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import TrackList from "../components/TrackList.svelte";

  let { tab }: { tab: LibraryTab } = $props();

  const tabs: { id: LibraryTab; label: string }[] = [
    { id: "playlists", label: "Playlists" },
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
    <button class="refresh" onclick={() => loadLibrary(tab, true)} disabled={slot.loading}>
      Actualiser
    </button>
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
    <TrackList tracks={library.tracks.items ?? []} />
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
