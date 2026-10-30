<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type AlbumSummary, type HomeSection } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { resetHomeLayout, saveHomeLayout, ui } from "../lib/stores/ui.svelte";
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
  const customized = $derived(ui.settings.home.order.length > 0 || ui.settings.home.hidden.length > 0);

  /** Premier album affiché, mis en avant avec le titre de sa ligne. */
  const featured = $derived.by((): { album: AlbumSummary; context: string } | null => {
    for (const row of visible) {
      const item = row.items.find((i) => i.kind === "album");
      if (item?.kind === "album") return { album: item, context: row.title };
    }
    return null;
  });

  onMount(async () => {
    if (rows) return;
    try {
      rows = cached = await commands.home();
      cachedLang = i18n.lang;
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

  async function reset() {
    try {
      await resetHomeLayout();
      // L'ordre du flux n'est connu que du backend : on recharge l'accueil.
      rows = cached = await commands.home();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<script lang="ts" module>
  let cached: HomeSection[] | null = null;
  let cachedLang: string | null = null;
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
    <HomeEditor sections={rows} canReset={customized} onchange={change} onreset={reset} />
  {:else}
    {#if featured}<Hero album={featured.album} context={featured.context} />{/if}
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
