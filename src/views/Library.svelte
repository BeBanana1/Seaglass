<script lang="ts">
  import { commands, type MediaItem, type PlaySource, type Track } from "../lib/ipc";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import BlindTestButton from "../components/BlindTestButton.svelte";
  import { go, replace, type LibraryTab } from "../lib/stores/nav.svelte";
  import {
    filterTracks,
    LIKED_DEFAULT_SORT,
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
  import CleanupDialog from "../components/CleanupDialog.svelte";
  import FolderCards from "../components/FolderCards.svelte";
  import { folders, loadFolders, rootPlaylists } from "../lib/stores/folders.svelte";
  import { listSort, saveListSort } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { listSearch } from "../lib/stores/listSearch.svelte";

  // Titres likés : l'API les renvoie du plus récent au plus ancien (date du like).
  const TRACK_SORT_KEYS: SortKey[] = ["added", "title", "artist", "album", "duration"];
  const TRACK_DEFAULT_SORT = LIKED_DEFAULT_SORT;

  let { tab }: { tab: LibraryTab } = $props();

  let query = $state("");
  // « Nettoyer » les titres likés (#382).
  let cleaning = $state(false);
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

  // « Plus tard » (#196) : les titres d'abord (liste lisible), puis les cartes.
  const laterTracks = $derived<Track[]>(
    (library.later.items ?? []).flatMap((item) => (item.kind === "track" ? [item] : [])),
  );
  const laterCards = $derived<MediaItem[]>(
    (library.later.items ?? []).flatMap((item) => (item.kind === "track" ? [] : [item])),
  );

  // Lecture / Aléatoire (#161) : comme l'en-tête d'une playlist, ils suivent le tri mais
  // pas le filtre.
  async function playAll(shuffle: boolean) {
    const later = tab === "later";
    const ids = later ? laterTracks.map((t) => t.id) : sortedTracks.map((l) => l.track.id);
    const source: PlaySource = later ? { kind: "later" } : { kind: "likedTracks" };
    try {
      await commands.playTracks(ids, 0, shuffle, source);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  // Sur Android, les titres likés ont leur onglet dans la navigation du bas (#143) : page à
  // part, sans les onglets de la bibliothèque, et sans onglet « Titres » en doublon.
  const likesPage = $derived(platform.mobile && tab === "tracks");
  const tabs = $derived<LibraryTab[]>(
    platform.mobile
      ? ["playlists", "mixes", "albums", "artists", "later"]
      : ["playlists", "mixes", "albums", "artists", "tracks", "later"],
  );

  // « Chargement… » seulement si l'onglet tarde : la copie du lancement précédent arrive
  // en quelques millisecondes et ne doit pas être précédée d'un clignotement (#181).
  const LOADING_DELAY_MS = 300;
  let slow = $state(false);

  $effect(() => {
    void tab;
    slow = false;
    const timer = setTimeout(() => (slow = true), LOADING_DELAY_MS);
    loadLibrary(tab);
    // Dossiers de playlists (#331) : une petite requête, rien d'affiché s'il n'y en a pas.
    if (tab === "playlists") loadFolders();
    return () => clearTimeout(timer);
  });

  const slot = $derived(library[tab]);

  const cards = $derived.by((): MediaItem[] => {
    switch (tab) {
      case "playlists":
        // Les playlists rangées dans un dossier n'apparaissent que dans leur dossier (#331).
        return rootPlaylists(library.playlists.items ?? []).map((p) => ({ kind: "playlist", ...p }));
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
        {#if tab === "tracks"}
          <button class="refresh" onclick={() => (cleaning = true)}>{t.views.cleanup.action}</button>
        {/if}
        {#if tab === "later"}<PinButton pin={{ kind: "later" }} />{/if}
        <!-- Statistiques d'écoute locales (#326). -->
        {#if !likesPage}<button class="refresh" onclick={() => go({ name: "stats" })}>{t.library.stats.open}</button>{/if}
        <button
          class="refresh"
          onclick={() => {
            loadLibrary(tab, true);
            if (tab === "playlists") loadFolders(true);
          }}
          disabled={slot.loading}
        >
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
    {#if slow}<p class="muted">{t.common.loading}</p>{/if}
  {:else if slot.items.length === 0 && !(tab === "playlists" && folders.items?.length)}
    <p class="muted">{tab === "later" ? t.library.later.empty : t.library.empty}</p>
  {:else}
    {#if slot.truncated && tab === "later"}
      <p class="library-truncated" role="status">{t.library.later.truncated(slot.items.length)}</p>
    {:else if slot.truncated && tab !== "mixes" && tab !== "later"}
      <!-- Plafond de chargement atteint (#91) : on le dit plutôt que de tronquer en silence. -->
      <p class="library-truncated" role="status">
        {t.library.truncated(tab, slot.items.length, slot.total)}
      </p>
    {/if}
    {@render content()}
  {/if}
  {#if cleaning}
    <CleanupDialog
      title={t.views.cleanup.likedTitle}
      positions={false}
      load={commands.likedCleanup}
      apply={commands.cleanLiked}
      undo={commands.cleanLiked}
      onClose={() => (cleaning = false)}
      onChanged={() => loadLibrary("tracks", true)}
    />
  {/if}
</section>

{#snippet playButtons()}
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
        {#if tab === "tracks"}
          <BlindTestButton title={t.library.likedTracks} trackIds={sortedTracks.map((l) => l.track.id)} />
        {/if}
      </div>
    </div>
  </header>
{/snippet}

{#snippet content()}
  {#if tab === "later"}
    {#if laterTracks.length > 0}
      {@render playButtons()}
      <TrackList tracks={laterTracks} source={{ kind: "later" }} />
    {/if}
    {#if laterCards.length > 0}
      {#if laterTracks.length > 0}<h2 class="later-others">{t.library.later.others}</h2>{/if}
      <CardGrid items={laterCards} />
    {/if}
  {:else if tab === "tracks"}
    {#if !listSearch.active}
      {@render playButtons()}
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
    {#if tab === "playlists" && folders.items}
      <FolderCards items={folders.items} playlists={library.playlists.items ?? []} />
    {/if}
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

  .later-others {
    margin: 28px 10px 12px;
    font-size: 18px;
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
