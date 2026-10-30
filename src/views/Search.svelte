<script lang="ts" module>
  import type { SearchResults } from "../lib/ipc";

  // Conservé au niveau du module : revenir sur la recherche retrouve la dernière requête.
  const saved = $state<{ query: string; results: SearchResults | null; allTracks: boolean }>({
    query: "",
    results: null,
    allTracks: false,
  });
</script>

<script lang="ts">
  import { commands, type MediaItem } from "../lib/ipc";
  import CardGrid from "../components/CardGrid.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { t } from "../lib/i18n.svelte";

  const COLLAPSED_TRACKS = 8;

  let searching = $state(false);
  let error = $state<string | null>(null);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    searching = true;
    error = null;
    try {
      saved.results = await commands.search(saved.query);
      saved.allTracks = false;
    } catch (err) {
      error = String(err);
    } finally {
      searching = false;
    }
  }

  const r = $derived(saved.results);
  const tracks = $derived(
    r ? (saved.allTracks ? r.tracks : r.tracks.slice(0, COLLAPSED_TRACKS)) : [],
  );
  const artists = $derived<MediaItem[]>(r?.artists.map((a) => ({ kind: "artist", ...a })) ?? []);
  const albums = $derived<MediaItem[]>(r?.albums.map((a) => ({ kind: "album", ...a })) ?? []);
  const playlists = $derived<MediaItem[]>(
    r?.playlists.map((p) => ({ kind: "playlist", ...p })) ?? [],
  );
  const empty = $derived(
    r !== null && !r.tracks.length && !r.artists.length && !r.albums.length && !r.playlists.length,
  );
</script>

<section>
  <form onsubmit={submit}>
    <!-- svelte-ignore a11y_autofocus -->
    <input
      type="search"
      placeholder={t.views.search.placeholder}
      bind:value={saved.query}
      autocomplete="off"
      spellcheck="false"
      autofocus
    />
  </form>

  {#if error}
    <p class="error">{error}</p>
  {:else if searching}
    <p class="muted">{t.views.search.searching}</p>
  {:else if empty}
    <p class="muted">{t.views.search.noResults}</p>
  {/if}

  {#if r && r.tracks.length > 0}
    <div class="tracks">
      <h2>{t.views.search.tracks}</h2>
      <!-- Le contexte de lecture est la liste affichée. -->
      <TrackList {tracks} />
      {#if r.tracks.length > COLLAPSED_TRACKS}
        <button class="more" onclick={() => (saved.allTracks = !saved.allTracks)}>
          {saved.allTracks ? t.views.search.showLess : t.views.search.showAll(r.tracks.length)}
        </button>
      {/if}
    </div>
  {/if}
  <CardGrid title={t.views.search.artists} items={artists} row />
  <CardGrid title={t.views.search.albums} items={albums} row />
  <CardGrid title={t.views.search.playlists} items={playlists} row />
</section>

<style>
  section {
    padding: 20px 24px;
  }

  form {
    margin: 0 10px 20px;
  }

  input {
    width: 100%;
    max-width: 520px;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg-raised);
    user-select: text;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  :global([data-theme="studio"]) input {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) input {
    border-radius: 0;
    border-width: 0 0 1px;
    border-color: var(--text);
    background: transparent;
    font-family: var(--display-family);
    font-size: 20px;
  }

  .tracks {
    margin-bottom: 28px;
  }

  h2 {
    margin: 0 10px 8px;
    font-size: 17px;
    font-weight: 600;
  }

  .more {
    margin: 6px 10px 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .more:hover {
    color: var(--text);
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
