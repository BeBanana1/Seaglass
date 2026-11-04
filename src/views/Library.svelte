<script lang="ts">
  import { commands, type MediaItem } from "../lib/ipc";
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
  import Icon from "../components/Icon.svelte";
  import { listSort, saveListSort } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { listSearch } from "../lib/stores/listSearch.svelte";

  // Titres likés : l'API les renvoie du plus récent au plus ancien (date du like).
  const TRACK_SORT_KEYS: SortKey[] = ["added", "title", "artist", "album", "duration"];
  const TRACK_DEFAULT_SORT: TrackSort = { key: "added", descending: true };

  let { tab }: { tab: LibraryTab } = $props();

  let query = $state("");
  // Tri mémorisé (#93), lu à l'ouverture de la bibliothèque.
  let sort = $state<TrackSort>(listSort("likedTracks", TRACK_DEFAULT_SORT));

  function setSort(next: TrackSort) {
    sort = next;
    saveListSort("likedTracks", next).catch((e) => pushToast({ level: "error", message: String(e) }));
  }

  const allTracks = $derived(library.tracks.items ?? []);
  // Le contexte de lecture est la liste affichée, dans l'ordre affiché.
  const sortedTracks = $derived(sortTracks(listTracks(allTracks), sort));
  const shownTracks = $derived(filterTracks(sortedTracks, query));

  // Lecture / Aléatoire (#161) : comme l'en-tête d'une playlist, ils suivent le tri mais
  // pas le filtre.
  async function playAll(shuffle: boolean) {
    try {
      await commands.playTracks(
        sortedTracks.map((l) => l.track.id),
        0,
        shuffle,
        { kind: "likedTracks" },
      );
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  // Sur Android, les titres likés ont leur onglet dans la navigation du bas (#143) : page à
  // part, sans les onglets de la bibliothèque, et sans onglet « Titres » en doublon.
  const likesPage = $derived(platform.mobile && tab === "tracks");
  const tabs = $derived<LibraryTab[]>(
    platform.mobile ? ["playlists", "mixes", "albums", "artists"] : ["playlists", "mixes", "albums", "artists", "tracks"],
  );

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
  <!-- Android : titre et onglets masqués pendant une recherche dans les titres likés (#151). -->
  {#if !listSearch.active}
    <div class="top">
      <h1>{likesPage ? t.library.likedTracks : t.library.title}</h1>
      <div class="top-actions">
        {#if tab === "tracks"}<PinButton pin={{ kind: "likedTracks" }} />{/if}
        <button class="refresh" onclick={() => loadLibrary(tab, true)} disabled={slot.loading}>
          {t.library.refresh}
        </button>
      </div>
    </div>
    {#if likesPage}
      <div class="likes-gap"></div>
    {:else}
      <div class="tabs" role="tablist">
        {#each tabs as id (id)}
          <button
            role="tab"
            aria-selected={id === tab}
            class:active={id === tab}
            onclick={() => replace({ name: "library", tab: id })}
          >
            {t.library.tabs[id]}
            {#if library[id].items}<span class="count"
                >{library[id].items?.length}{library[id].truncated ? "+" : ""}</span
              >{/if}
          </button>
        {/each}
      </div>
    {/if}
  {/if}

  {#if slot.error}
    <p class="error">{slot.error}</p>
  {:else if !slot.items}
    <p class="muted">{t.common.loading}</p>
  {:else if slot.items.length === 0}
    <p class="muted">{t.library.empty}</p>
  {:else}
    {#if slot.truncated && tab !== "mixes"}
      <!-- Plafond de chargement atteint (#91) : on le dit plutôt que de tronquer en silence. -->
      <p class="library-truncated" role="status">
        {t.library.truncated(tab, slot.items.length, slot.total)}
      </p>
    {/if}
    {@render content()}
  {/if}
</section>

{#snippet content()}
  {#if tab === "tracks"}
    {#if !listSearch.active}
      <!-- Mêmes classes que l'en-tête d'une page (`header .info .actions`) : les thèmes
           habillent ces boutons comme ceux d'une playlist. -->
      <header class="liked-header">
        <div class="info">
          <div class="actions">
            <button class="primary" onclick={() => playAll(false)}>
              <Icon name="play" size={16} /> {t.common.play}
            </button>
            <button class="secondary" onclick={() => playAll(true)}>
              <Icon name="shuffle" size={16} /> {t.common.shuffle}
            </button>
          </div>
        </div>
      </header>
    {/if}
    <TrackTools
      bind:query
      bind:sort={() => sort, setSort}
      keys={TRACK_SORT_KEYS}
      shown={shownTracks.length}
      total={allTracks.length}
    />
    {#if shownTracks.length === 0}
      <p class="muted">{t.library.noMatch(query.trim())}</p>
    {:else}
      <TrackList tracks={shownTracks.map((l) => l.track)} source={{ kind: "likedTracks" }} />
    {/if}
  {:else}
    <CardGrid items={cards} />
  {/if}
{/snippet}

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

  /* Page des titres likés sur Android : l'espace que laissaient les onglets. */
  .likes-gap {
    height: 16px;
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

  /* Lecture / Aléatoire des titres likés : styles des boutons de `PageHeader`. */
  .liked-header {
    margin: 0 10px 18px;
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

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }

  .library-truncated {
    margin: 0 10px 14px;
    padding: 8px 12px;
    border-left: 3px solid var(--accent);
    border-radius: 6px;
    font-size: 13px;
    color: var(--text-muted);
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  /* Android (#24) : onglets sur une ligne, qui défile au doigt. */
  :global(:root[data-platform="android"]) .tabs {
    overflow-x: auto;
    scrollbar-width: none;
  }

  :global(:root[data-platform="android"]) .tabs button {
    flex-shrink: 0;
    white-space: nowrap;
  }
</style>
