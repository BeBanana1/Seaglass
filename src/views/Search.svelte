<script lang="ts" module>
  import type { SearchResults } from "../lib/ipc";

  // Conservé au niveau du module : revenir sur la recherche retrouve la dernière requête.
  const saved = $state<{
    query: string;
    /** Requête des résultats affichés (le champ a pu changer depuis). */
    searched: string;
    results: SearchResults | null;
    allTracks: boolean;
  }>({
    query: "",
    searched: "",
    results: null,
    allTracks: false,
  });
</script>

<script lang="ts">
  import { commands, type MediaItem } from "../lib/ipc";
  import CardGrid from "../components/CardGrid.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { t } from "../lib/i18n.svelte";
  import { openTidalLink } from "../lib/links";

  const COLLAPSED_TRACKS = 8;
  /** Pause de frappe avant de chercher (#248), comme la page de la Jam. */
  const TYPING_DELAY = 400;

  let searching = $state(false);
  let error = $state<string | null>(null);
  /** Dernières recherches (#211), tenues par le backend. */
  let history = $state<string[]>([]);
  let input = $state<HTMLInputElement>();
  let typingTimer: ReturnType<typeof setTimeout> | undefined;
  /** Numéro de la dernière recherche lancée : une réponse plus ancienne est ignorée. */
  let seq = 0;
  /** Requête déjà gardée dans l'historique, pour ne pas la renvoyer à chaque clic. */
  let noted = "";

  function refreshHistory() {
    commands.searchHistory().then(
      (h) => (history = h),
      () => {},
    );
  }
  refreshHistory();

  /** Garde `query` dans l'historique : seulement une requête retenue, pas chaque préfixe tapé. */
  function remember(query: string) {
    const q = query.trim();
    if (!q || q === noted) return;
    noted = q;
    commands.noteSearchHistory(q).then(
      (h) => (history = h),
      () => {},
    );
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    clearTimeout(typingTimer);
    // Un lien TIDAL ouvre sa page au lieu d'être cherché (#203).
    if (await openTidalLink(saved.query)) return;
    // Résultats déjà là pour cette requête : Entrée ne fait que la retenir.
    if (saved.results && saved.searched === saved.query.trim() && !error) {
      remember(saved.query);
      return;
    }
    await run(saved.query, true);
  }

  /** Recherche pendant la frappe (#248), après une courte pause. */
  function typed() {
    clearTimeout(typingTimer);
    const query = saved.query;
    if (!query.trim()) {
      // Champ vidé : l'historique revient, une réponse en route est oubliée.
      seq++;
      searching = false;
      error = null;
      return;
    }
    // Un lien collé est traité par `onpaste`, pas cherché comme du texte.
    if (/^\s*https?:\/\//i.test(query)) return;
    typingTimer = setTimeout(() => void run(query, false), TYPING_DELAY);
  }

  async function run(query: string, keep: boolean) {
    clearTimeout(typingTimer);
    const id = ++seq;
    saved.query = query;
    searching = true;
    error = null;
    try {
      const results = await commands.search(query);
      if (id !== seq) return;
      saved.results = results;
      saved.searched = query.trim();
      saved.allTracks = false;
      if (keep) remember(query);
    } catch (err) {
      if (id === seq) error = String(err);
    } finally {
      if (id === seq) searching = false;
    }
  }

  async function removeEntry(query: string) {
    try {
      history = await commands.removeSearchHistory(query);
    } catch {
      // Rien à signaler : l'entrée reste affichée.
    }
    input?.focus();
  }

  async function clearHistory() {
    try {
      history = await commands.clearSearchHistory();
    } catch {
      // Idem.
    }
    input?.focus();
  }

  // Champ vide : les recherches récentes à la place des résultats.
  const showHistory = $derived(!saved.query.trim() && !searching);

  const r = $derived(showHistory ? null : saved.results);
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
      bind:this={input}
      oninput={typed}
      onpaste={(e) => {
        const text = e.clipboardData?.getData("text/plain");
        if (text) void openTidalLink(text);
      }}
      autocomplete="off"
      spellcheck="false"
      autofocus
    />
  </form>

  {#if showHistory && history.length > 0}
    <div class="search-history">
      <div class="search-history-head">
        <h2>{t.views.search.recent}</h2>
        <button class="search-history-clear" onclick={clearHistory}>
          {t.views.search.clearHistory}
        </button>
      </div>
      <ul>
        {#each history as query (query)}
          <li class="search-history-item">
            <button class="search-history-query" onclick={() => run(query, true)}>
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M12 7v5l3 2" />
                <circle cx="12" cy="12" r="8" />
              </svg>
              <span>{query}</span>
            </button>
            <button
              class="search-history-remove"
              aria-label={t.views.search.removeEntry(query)}
              title={t.views.search.removeEntry(query)}
              onclick={() => removeEntry(query)}
            >
              <svg viewBox="0 0 24 24" aria-hidden="true">
                <path d="M7 7l10 10M17 7L7 17" />
              </svg>
            </button>
          </li>
        {/each}
      </ul>
    </div>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {:else if searching && !r}
    <p class="muted">{t.views.search.searching}</p>
  {:else if empty && !searching}
    <p class="muted">{t.views.search.noResults}</p>
  {/if}

  <!-- Un clic dans les résultats retient la requête (#248) : c'est celle qu'on cherchait. -->
  <div class="results" class:stale={searching} onpointerdowncapture={() => remember(saved.searched)}>

  {#if r && r.tracks.length > 0}
    <div class="tracks">
      <h2>{t.views.search.tracks}</h2>
      <!-- Le contexte de lecture est la liste affichée. -->
      <TrackList {tracks} source={{ kind: "search", query: saved.searched }} />
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
  </div>
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

  /* Résultats de la requête précédente pendant que la suivante arrive : pas de saut. */
  .results {
    transition: opacity 0.15s;
  }

  .results.stale {
    opacity: 0.6;
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

  .search-history {
    max-width: 540px;
    margin-bottom: 24px;
  }

  .search-history-head {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 12px;
  }

  .search-history-clear {
    flex: none;
    margin-right: 10px;
    white-space: nowrap;
    font-size: 13px;
    color: var(--text-muted);
  }

  .search-history-clear:hover {
    color: var(--text);
  }

  .search-history ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .search-history-item {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
  }

  .search-history-item:hover {
    background: var(--bg-hover);
  }

  .search-history-query {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px;
    text-align: left;
  }

  .search-history-query span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .search-history svg {
    flex: none;
    width: 16px;
    height: 16px;
    fill: none;
    stroke: currentColor;
    stroke-width: 2;
    stroke-linecap: round;
  }

  .search-history-query svg {
    color: var(--text-muted);
  }

  .search-history-remove {
    flex: none;
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    margin-right: 4px;
    color: var(--text-muted);
    opacity: 0;
  }

  .search-history-item:hover .search-history-remove,
  .search-history-remove:focus-visible {
    opacity: 1;
  }

  .search-history-remove:hover {
    color: var(--text);
  }

  :global(:root[data-platform="android"]) .search-history-remove {
    opacity: 1;
    width: 44px;
    height: 44px;
  }

  :global(:root[data-platform="android"]) .search-history-query {
    padding: 12px 10px;
  }
</style>
