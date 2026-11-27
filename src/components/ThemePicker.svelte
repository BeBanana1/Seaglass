<!-- Theme gallery (Appearance section of the settings, #390): compact thumbnails, one per
     family (the light / dark variants are chosen in `CurrentTheme`), filters and
     favorites first. The description goes in a tooltip and in the current theme block. -->
<script lang="ts">
  import { setTheme, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import {
    familyOf,
    GALLERY_FILTERS,
    galleryFamilies,
    variantFor,
    type GalleryFilter,
  } from "../lib/themes";
  import type { Theme } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import ThemePreview from "./ThemePreview.svelte";

  let filter = $state<GalleryFilter>("all");
  let grid = $state<HTMLDivElement>();

  const current = $derived(ui.settings.theme);
  const currentFamily = $derived(familyOf(current));
  const favorites = $derived(ui.settings.favoriteThemes);
  const families = $derived(galleryFamilies(filter, favorites));
  // Tab key: the thumbnail of the current theme, otherwise the first one displayed.
  const tabStop = $derived(families.includes(currentFamily) ? currentFamily : families[0]);

  async function choose(theme: Theme) {
    if (theme === current) return;
    try {
      await setTheme(theme);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  /** Number of columns actually displayed, for Up / Down. */
  function columns(): number {
    if (!grid) return 1;
    return getComputedStyle(grid).gridTemplateColumns.split(" ").filter(Boolean).length || 1;
  }

  // The arrows move the focus without applying the theme (Space or Enter does):
  // applying a theme rebuilds its decorative background, too costly on every key press.
  function onKeydown(e: KeyboardEvent) {
    const cards = [...(grid?.querySelectorAll<HTMLButtonElement>(".theme-card") ?? [])];
    const from = cards.indexOf(document.activeElement as HTMLButtonElement);
    if (from < 0) return;
    const step: Record<string, number> = {
      ArrowLeft: -1,
      ArrowRight: 1,
      ArrowUp: -columns(),
      ArrowDown: columns(),
      Home: -from,
      End: cards.length - 1 - from,
    };
    if (!(e.key in step)) return;
    e.preventDefault();
    const to = Math.min(cards.length - 1, Math.max(0, from + step[e.key]));
    cards[to].focus();
  }
</script>

<div class="cache-sizes theme-filters" role="radiogroup" aria-label={t.settings.themeFilter}>
  {#each GALLERY_FILTERS as f (f)}
    <button
      class="cache-size"
      class:selected={filter === f}
      role="radio"
      aria-checked={filter === f}
      onclick={() => (filter = f)}
    >
      {t.settings.themeFilters[f]}
    </button>
  {/each}
</div>

{#if families.length === 0}
  <p class="empty">{t.settings.themeNoFavorites}</p>
{:else}
  <!-- svelte-ignore a11y_interactive_supports_focus -->
  <div
    class="themes"
    role="radiogroup"
    aria-label={t.settings.themeGallery}
    bind:this={grid}
    onkeydown={onKeydown}
  >
    {#each families as family (family)}
      {@const theme = variantFor(family, current, filter)}
      {@const selected = family === currentFamily}
      <button
        class="theme-card"
        class:selected
        role="radio"
        aria-checked={selected}
        tabindex={family === tabStop ? 0 : -1}
        title={t.settings.themes[theme].description}
        onclick={() => choose(theme)}
      >
        <ThemePreview {theme} />
        <span class="name">
          <span class="label">{t.settings.themes[family].name}</span>
          {#if favorites.includes(family)}
            <Icon name="starFilled" size={13} />
          {/if}
        </span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .theme-filters {
    margin-bottom: 14px;
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(124px, 1fr));
    gap: 10px;
  }

  .theme-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
    padding: 6px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    text-align: left;
  }

  .theme-card:hover {
    background: var(--bg-hover);
  }

  .theme-card.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .name {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 0 2px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
  }

  .name :global(svg) {
    flex-shrink: 0;
    color: var(--accent);
  }

  .label {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .empty {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  /* Same selectors as those of the settings (theme styles included). */
  .cache-sizes {
    display: inline-flex;
    flex-wrap: wrap;
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .cache-size {
    padding: 6px 14px;
    border-radius: var(--radius-pill);
    font-size: 13px;
    color: var(--text-muted);
  }

  .cache-size:hover:not(.selected) {
    color: var(--text);
    background: var(--bg-hover);
  }

  .cache-size.selected {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  /* Phone: three thumbnails per row. */
  :global(:root[data-platform="android"]) .themes {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 8px;
  }

  :global([data-theme="studio"]) .theme-card {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .theme-card {
    border-radius: 2px;
  }
</style>
