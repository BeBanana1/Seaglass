<script lang="ts">
  import { onMount } from "svelte";
  import { commands, PLAY_HISTORY_KEY, type AlbumSummary, type HomeSection } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { resetHomeLayout, saveHomeLayout, setHomeFeatured, ui } from "../lib/stores/ui.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import Hero from "../components/Hero.svelte";
  import HomeEditor from "../components/HomeEditor.svelte";
  import { i18n, t } from "../lib/i18n.svelte";

  // Gardé entre deux visites de l'accueil pour éviter de recharger à chaque retour, tant
  // que la langue ne change pas (les titres des sections viennent du service).
  if (cachedLang !== i18n.lang) cached = null;
  let rows = $state<HomeSection[] | null>(cached);
  let error = $state<string | null>(null);
  let editing = $state(false);

  const visible = $derived((rows ?? []).filter((r) => !r.hidden));
  const customized = $derived(
    ui.settings.home.order.length > 0 || ui.settings.home.hidden.length > 0 || ui.settings.home.featuredHidden,
  );

  /** Albums les plus écoutés dans l'app (#202), relus à chaque visite. */
  let topPlayed = $state<AlbumSummary[]>([]);

  /** Albums les plus écoutés, puis ceux des sections affichées, sans doublon, avec le titre
   *  de leur ligne (#185, #202). */
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

  /** Album mis en avant : tiré au hasard une fois par lancement, puis « Passer » avance.
   * Une fois montré, il reste le même quand la suite de l'accueil arrive (`featuredId`). */
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

  /** Première page, puis tout l'accueil s'il y a d'autres pages (#207). */
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
      // La première page reste affichée.
      console.warn("suite de l'accueil", e);
    }
  }

  /**
   * Historique d'écoute local (#202), relu à chaque visite : l'accueil de TIDAL reste gardé
   * en mémoire, mais « Écoutés dans Seaglass » suit les écoutes faites depuis. La section
   * n'apparaît qu'au chargement complet suivant si l'historique était vide.
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
      console.warn("historique d'écoute", e);
    }
  }

  onMount(async () => {
    if (rows) {
      refreshPlayHistory();
      return;
    }
    try {
      await Promise.all([load(), commands.playHistory().then((h) => (topPlayed = h.top)).catch(() => {})]);
    } catch (e) {
      error = String(e);
    }
  });

  /** Affiche tout de suite la nouvelle disposition, puis l'enregistre (retour arrière si échec). */
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
      // L'ordre du flux n'est connu que du backend : on recharge l'accueil.
      await load();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<script lang="ts" module>
  let cached: HomeSection[] | null = null;
  let cachedLang: string | null = null;
  // Gardé entre deux visites de l'accueil : le même album tant que l'app reste ouverte.
  // Hors de toute ligne (`$state` de module) pour que « Passer » rafraîchisse la vue.
  let featuredIndex = $state(Math.floor(Math.random() * 1000));
  /** Album affiché, gardé quand la liste des candidats change. */
  let featuredId = $state<number | null>(null);
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
