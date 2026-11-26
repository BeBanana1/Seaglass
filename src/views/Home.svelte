<script lang="ts" module>
  let cached: HomeSection[] | null = null;
  let cachedLang: string | null = null;
  // Kept between two visits to the home page: the same album as long as the app stays open.
  // Outside any row (module-level `$state`) so that "Skip" refreshes the view.
  let featuredIndex = $state(Math.floor(Math.random() * 1000));
  /** Album shown, kept when the list of candidates changes. */
  let featuredId = $state<number | null>(null);
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { commands, PLAY_HISTORY_KEY, type AlbumSummary, type HomeSection } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { watchHistory } from "../lib/stores/handoff.svelte";
  import { resetHomeLayout, saveHomeLayout, setHomeFeatured, ui } from "../lib/stores/ui.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import Hero from "../components/Hero.svelte";
  import HomeEditor from "../components/HomeEditor.svelte";
  import { i18n, t } from "../lib/i18n.svelte";

  // Kept between two visits to the home page to avoid reloading on every return, as long
  // as the language does not change (section titles come from the service).
  if (cachedLang !== i18n.lang) cached = null;
  let rows = $state<HomeSection[] | null>(cached);
  let error = $state<string | null>(null);
  let editing = $state(false);

  const visible = $derived((rows ?? []).filter((r) => !r.hidden));
  const customized = $derived(
    ui.settings.home.order.length > 0 ||
      ui.settings.home.hidden.length > 0 ||
      ui.settings.home.featuredHidden,
  );

  /** Most played albums in the app (#202), reread on every visit. */
  let topPlayed = $state<AlbumSummary[]>([]);

  /** Most played albums, then those of the sections shown, without duplicates, with the title
   *  of their row (#185, #202). */
  const candidates = $derived.by(() => {
    const seen = new Set<number>();
    const list: { album: AlbumSummary; context: string }[] = [];
    for (const album of topPlayed) {
      if (seen.has(album.id)) continue;
      seen.add(album.id);
      list.push({ album, context: t.views.home.mostPlayed });
    }
    for (const row of visible) {
      for (const item of row.items) {
        if (item.kind !== "album" || seen.has(item.id)) continue;
        seen.add(item.id);
        list.push({ album: item, context: row.title });
      }
    }
    return list;
  });

  /** Featured album: picked at random once per launch, then "Skip" advances.
   * Once shown, it stays the same when the rest of the home page arrives (`featuredId`). */
  const featured = $derived(
    ui.settings.home.featuredHidden || candidates.length === 0
      ? null
      : (candidates.find((c) => c.album.id === featuredId) ??
          candidates[featuredIndex % candidates.length]),
  );

  function skipFeatured() {
    const current = featured ? candidates.indexOf(featured) : -1;
    featuredId = candidates[(current + 1) % candidates.length].album.id;
  }

  /** First page, then the whole home page if there are more pages (#207). */
  async function load() {
    const feed = await commands.home();
    rows = cached = feed.sections;
    cachedLang = i18n.lang;
    if (!feed.more) return;
    try {
      const all = await commands.homeRest();
      featuredId = featured?.album.id ?? featuredId;
      rows = cached = all;
    } catch (e) {
      // The first page stays shown.
      console.warn("suite de l'accueil", e);
    }
  }

  /**
   * Local listening history (#202), reread on every visit: TIDAL's home page stays kept
   * in memory, but "Listened to in Seaglass" follows the plays made since. The section
   * only appears at the next full load if the history was empty.
   */
  async function refreshPlayHistory() {
    try {
      const history = await commands.playHistory();
      topPlayed = history.top;
      if (!rows) return;
      const items = history.recent.map((a) => ({ kind: "album" as const, ...a }));
      rows = cached = rows
        .map((r) => (r.key === PLAY_HISTORY_KEY ? { ...r, items } : r))
        .filter((r) => r.key !== PLAY_HISTORY_KEY || r.items.length > 0);
    } catch (e) {
      console.warn("listening history", e);
    }
  }

  // Plays received from the linked device (#430): the sync at launch is enough here.
  onMount(() => watchHistory(refreshPlayHistory, false));

  onMount(async () => {
    if (rows) {
      refreshPlayHistory();
      return;
    }
    try {
      await Promise.all([
        load(),
        commands
          .playHistory()
          .then((h) => (topPlayed = h.top))
          .catch(() => {}),
      ]);
    } catch (e) {
      error = String(e);
    }
  });

  /** Shows the new layout right away, then saves it (rolls back on failure). */
  async function change(next: HomeSection[]) {
    const previous = rows;
    rows = cached = next;
    try {
      await saveHomeLayout(next);
    } catch (e) {
      rows = cached = previous;
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function toggleFeatured(shown: boolean) {
    try {
      await setHomeFeatured(shown);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function reset() {
    try {
      await resetHomeLayout();
      // Only the backend knows the order of the feed: reload the home page.
      await load();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<section>
  <div class="home-head">
    <h1>{t.views.home.title}</h1>
    {#if rows}
      <button
        class="home-customize"
        class:active={editing}
        aria-expanded={editing}
        onclick={() => (editing = !editing)}
      >
        {editing ? t.views.home.done : t.views.home.customize}
      </button>
    {/if}
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if !rows}
    <p class="muted">{t.common.loading}</p>
  {:else if editing}
    <HomeEditor
      sections={rows}
      canReset={customized}
      featuredHidden={ui.settings.home.featuredHidden}
      onchange={change}
      onfeatured={toggleFeatured}
      onreset={reset}
    />
  {:else}
    {#if featured}
      <Hero
        album={featured.album}
        context={featured.context}
        onskip={candidates.length > 1 ? skipFeatured : undefined}
      />
    {/if}
    {#each visible as row, i (`${row.key}-${i}`)}
      <CardGrid title={row.title} items={row.items} row />
    {/each}
    {#if rows.length > 0 && visible.length === 0}
      <p class="muted">{t.views.home.allHidden}</p>
    {/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .home-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin: 0 10px 20px;
  }

  h1 {
    margin: 0;
    font-size: 26px;
  }

  .home-customize {
    flex-shrink: 0;
    padding: 6px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--text-muted);
    background: var(--control-bg, none);
    box-shadow: var(--shadow-raised);
  }

  .home-customize:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .home-customize.active {
    border-color: transparent;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .home-customize.active:hover {
    background: var(--accent-strong);
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }

  :global([data-theme="studio"]) .home-customize {
    font-size: 12px;
  }
</style>
