<script lang="ts" module>
  import type { SearchResults } from "../lib/ipc";

  // Kept at module level: coming back to search finds the last query.
  const saved = $state<{
    query: string;
    /** Query of the displayed results (the field may have changed since). */
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
  import { ui } from "../lib/stores/ui.svelte";
  import { openTidalLink } from "../lib/links";
  import { eggs, searched } from "../lib/eggs/eggs.svelte";

  const COLLAPSED_TRACKS = 8;
  /** Typing pause before searching (#248), like the Jam page. */
  const TYPING_DELAY = 400;

  let searching = $state(false);
  let error = $state<string | null>(null);
  /** Latest searches (#211), kept by the backend. */
  let history = $state<string[]>([]);
  let input = $state<HTMLInputElement>();
  let typingTimer: ReturnType<typeof setTimeout> | undefined;
  /** Number of the latest search started: an older response is ignored. */
  let seq = 0;
  /** Query already kept in the history, so it is not sent again on every click. */
  let noted = "";

  function refreshHistory() {
    commands.searchHistory().then(
      (h) => (history = h),
      () => {},
    );
  }
  refreshHistory();

  /** Keeps `query` in the history: only a query that was settled on, not every prefix typed. */
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
    // A TIDAL link opens its page instead of being searched (#203).
    if (await openTidalLink(saved.query)) return;
    // Results already there for this query: Enter only keeps it.
    if (saved.results && saved.searched === saved.query.trim() && !error) {
      remember(saved.query);
      return;
    }
    await run(saved.query, true);
  }

  /** Search while typing (#248), after a short pause. */
  function typed() {
    clearTimeout(typingTimer);
    const query = saved.query;
    if (!query.trim()) {
      // Field cleared: the history comes back, a response on its way is forgotten.
      seq++;
      searching = false;
      error = null;
      return;
    }
    // A pasted link is handled by `onpaste`, not searched as text.
    if (/^\s*https?:\/\//i.test(query)) return;
    typingTimer = setTimeout(() => void run(query, false), TYPING_DELAY);
  }

  async function run(query: string, keep: boolean) {
    clearTimeout(typingTimer);
    const id = ++seq;
    saved.query = query;
    searching = true;
    error = null;
    // #399: a specific word plays a brief easter egg, the search still goes ahead.
    searched(query);
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
      // Nothing to report: the entry stays shown.
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

  // Empty field: recent searches instead of results.
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
    <!-- #399: brief effects for some searches, over and around the field. -->
    {#if eggs.search === "rosebud"}
      <span class="egg flakes" aria-hidden="true">
        {#each [8, 21, 34, 47, 60, 73, 86] as left, i (left)}
          <i style="left: {left}%; animation-delay: {(i % 3) * 0.2}s"></i>
        {/each}
      </span>
    {:else if eggs.search === "mellon"}
      <svg class="egg arch" viewBox="0 0 100 30" preserveAspectRatio="none" aria-hidden="true">
        <path d="M2 30V18C2 8 30 1 50 1s48 7 48 17v12" pathLength="1" />
      </svg>
    {:else if eggs.search === "spice"}
      <span class="egg grains" aria-hidden="true">
        {#each [12, 25, 38, 52, 66, 80] as left, i (left)}
          <i style="left: {left}%; animation-delay: {i * 0.12}s"></i>
        {/each}
      </span>
    {/if}
  </form>
  {#if eggs.search === "answer"}
    <p class="egg-line">{t.eggs.answer}</p>
  {/if}
  {#if eggs.search === "origami"}
    <!-- A small folded unicorn, placed in a corner. -->
    <svg class="egg origami" viewBox="0 0 40 40" width="40" height="40" aria-hidden="true">
      <path d="M8 34 14 20l8 2z" fill="#d9d4c7" />
      <path d="M14 20 22 22l6-10z" fill="#bdb6a6" />
      <path d="M22 22 28 12l6 2z" fill="#e8e3d6" />
      <path d="M28 12l4-8 1 9z" fill="#a8a190" />
      <path d="M22 22l2 12h-4z" fill="#bdb6a6" />
    </svg>
  {/if}

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
    <p class="muted">
      {ui.settings.theme === "noir" ? t.views.search.noResultsNoir : t.views.search.noResults}
    </p>
  {/if}

  <!-- A click in the results keeps the query (#248): it is the one that was looked for. -->
  <div
    class="results"
    class:stale={searching}
    onpointerdowncapture={() => remember(saved.searched)}
  >
    {#if r && r.tracks.length > 0}
      <div class="tracks">
        <h2>{t.views.search.tracks}</h2>
        <!-- The playback context is the displayed list. -->
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
    position: relative;
    max-width: 520px;
    margin: 0 10px 20px;
  }

  /* Easter eggs (#399). None blocks the field or replays. */
  .egg {
    position: absolute;
    pointer-events: none;
  }

  .flakes,
  .grains {
    inset: 0;
    overflow: hidden;
    border-radius: 999px;
  }

  .flakes i {
    position: absolute;
    top: -4px;
    width: 3px;
    height: 3px;
    border-radius: 50%;
    background: var(--text);
    opacity: 0.7;
    animation: egg-fall 1.4s linear 1 forwards;
  }

  @keyframes egg-fall {
    to {
      translate: 6px 44px;
      opacity: 0;
    }
  }

  .grains {
    inset: auto 0 -10px;
    height: 10px;
    overflow: visible;
  }

  .grains i {
    position: absolute;
    top: 0;
    width: 2px;
    height: 2px;
    border-radius: 50%;
    background: #c8743a;
    animation: egg-slide 1.5s ease-out 1 forwards;
  }

  @keyframes egg-slide {
    to {
      translate: 18px 6px;
      opacity: 0;
    }
  }

  /* "Spice" search: the accent turns cinnamon for the length of the effect. */
  :global(:root[data-egg="spice"]) {
    --accent: #c8743a !important;
  }

  .arch {
    left: -6px;
    right: -6px;
    bottom: 50%;
    width: calc(100% + 12px);
    height: 34px;
  }

  .arch path {
    fill: none;
    stroke: color-mix(in oklab, var(--accent) 70%, #fff);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
    filter: drop-shadow(0 0 3px var(--accent));
    stroke-dasharray: 1;
    stroke-dashoffset: 1;
    animation: egg-draw 1.2s ease-out 1 forwards;
  }

  @keyframes egg-draw {
    to {
      stroke-dashoffset: 0;
    }
  }

  .egg-line {
    margin: -12px 10px 16px 24px;
    font-size: 12px;
    font-style: italic;
    color: var(--text-muted);
  }

  .origami {
    position: fixed;
    right: 36px;
    bottom: 120px;
    animation: egg-fade 1.8s ease-in-out 1 forwards;
  }

  @keyframes egg-fade {
    0%,
    100% {
      opacity: 0;
    }
    25%,
    75% {
      opacity: 1;
    }
  }

  /* Reduced motion: the static variant, which goes away with the effect. */
  :global(:root[data-motion="reduced"]) .egg,
  :global(:root[data-motion="reduced"]) .egg :is(i, path) {
    animation: none;
  }

  :global(:root[data-motion="reduced"]) .arch path {
    stroke-dashoffset: 0;
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

  /* Results of the previous query while the next one arrives: no jump. */
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
