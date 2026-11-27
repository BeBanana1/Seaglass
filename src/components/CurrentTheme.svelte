<!-- Current theme (Appearance section of the settings, #390): large thumbnail, description,
     favorite, and the theme's own settings: light / dark variant and accent color. -->
<script lang="ts">
  import { setTheme, toggleFavoriteTheme, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { familyOf, themeInfo, variants } from "../lib/themes";
  import type { Theme } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";
  import AccentPicker from "./AccentPicker.svelte";
  import Icon from "./Icon.svelte";
  import ThemePreview from "./ThemePreview.svelte";

  const current = $derived(ui.settings.theme);
  const family = $derived(familyOf(current));
  const familyVariants = $derived(variants(family));
  const favorite = $derived(ui.settings.favoriteThemes.includes(family));
  const favoriteLabel = $derived(
    favorite ? t.settings.themeFavoriteRemove : t.settings.themeFavoriteAdd,
  );

  async function run(action: () => Promise<void>) {
    try {
      await action();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function chooseVariant(theme: Theme) {
    if (theme !== current) run(() => setTheme(theme));
  }
</script>

<div class="theme-current">
  <ThemePreview theme={current} size="large" />
  <div class="details">
    <div class="title">
      <span class="name">{t.settings.themes[family].name}</span>
      <button
        class="theme-favorite"
        class:on={favorite}
        aria-pressed={favorite}
        aria-label={favoriteLabel}
        title={favoriteLabel}
        onclick={() => run(() => toggleFavoriteTheme(family))}
      >
        <Icon name={favorite ? "starFilled" : "star"} size={20} />
      </button>
    </div>
    <p class="desc">{t.settings.themes[current].description}</p>

    {#if familyVariants.length > 1}
      <div class="row">
        <span class="label">{t.settings.themeVariant}</span>
        <div class="cache-sizes" role="radiogroup" aria-label={t.settings.themeVariant}>
          {#each familyVariants as variant (variant.id)}
            <button
              class="cache-size"
              class:selected={current === variant.id}
              role="radio"
              aria-checked={current === variant.id}
              onclick={() => chooseVariant(variant.id)}
            >
              {t.settings.themeSchemes[themeInfo(variant.id).scheme]}
            </button>
          {/each}
        </div>
      </div>
    {/if}

    <div class="row accent">
      <span class="label">{t.settings.accent}</span>
      <AccentPicker />
    </div>
    <p class="hint">{t.settings.accentHint}</p>
  </div>
</div>

<style>
  .theme-current {
    display: grid;
    grid-template-columns: 220px 1fr;
    gap: 20px;
    padding: 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .details {
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-width: 0;
  }

  .title {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .name {
    font-size: 18px;
    font-weight: 600;
  }

  .theme-favorite {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 34px;
    height: 34px;
    border-radius: var(--radius-pill);
    color: var(--text-muted);
  }

  .theme-favorite:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .theme-favorite.on {
    color: var(--accent);
  }

  .desc {
    margin: -8px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .row {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
  }

  .label {
    font-size: 13px;
    font-weight: 600;
  }

  .hint {
    margin: -4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Same selectors as the settings' (theme styles included). */
  .cache-sizes {
    display: inline-flex;
    gap: 4px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
  }

  .cache-size {
    min-width: 76px;
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

  @media (max-width: 640px) {
    .theme-current {
      grid-template-columns: 1fr;
    }

    .theme-current > :global(.preview) {
      max-width: 240px;
    }
  }

  :global([data-theme="studio"]) .theme-current {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .theme-current {
    border-radius: 2px;
  }
</style>
