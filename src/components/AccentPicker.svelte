<!-- Couleur d'accent (section Apparence des réglages) : celle du thème, une couleur proposée
     ou une couleur libre. Elle remplace l'accent de tous les thèmes. -->
<script lang="ts">
  import { ACCENT_PRESETS, setAccent, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";

  const current = $derived(ui.settings.accent);
  const custom = $derived(current !== null && !ACCENT_PRESETS.includes(current));

  async function choose(accent: string | null) {
    if (accent === current) return;
    try {
      await setAccent(accent);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  // `change` (pas `input`) : une seule écriture quand le sélecteur se ferme.
  function onPick(e: Event) {
    choose((e.currentTarget as HTMLInputElement).value.toLowerCase());
  }
</script>

<div class="accents" role="radiogroup" aria-label="Couleur d'accent">
  <button
    class="accent-theme"
    class:selected={current === null}
    role="radio"
    aria-checked={current === null}
    onclick={() => choose(null)}
  >
    Celle du thème
  </button>
  {#each ACCENT_PRESETS as color (color)}
    <button
      class="accent-swatch"
      class:selected={current === color}
      style:--swatch={color}
      role="radio"
      aria-checked={current === color}
      aria-label={color}
      title={color}
      onclick={() => choose(color)}
    ></button>
  {/each}
  <label class="accent-custom" class:selected={custom} title="Autre couleur">
    <input type="color" value={current ?? "#c8ff00"} onchange={onPick} aria-label="Autre couleur" />
    <span>{custom ? current : "Autre…"}</span>
  </label>
</div>

<style>
  .accents {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
  }

  .accent-theme,
  .accent-custom {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 13px;
    color: var(--text-muted);
    cursor: pointer;
  }

  .accent-theme:hover,
  .accent-custom:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .accent-theme.selected,
  .accent-custom.selected {
    color: var(--text);
    border-color: var(--accent);
  }

  .accent-swatch {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    background: var(--swatch);
    box-shadow: 0 0 0 1px rgb(0 0 0 / 0.25) inset;
  }

  .accent-swatch.selected {
    outline: 2px solid var(--text);
    outline-offset: 3px;
  }

  .accent-custom {
    padding-left: 6px;
  }

  .accent-custom input {
    width: 24px;
    height: 24px;
    padding: 0;
    border: none;
    border-radius: 50%;
    background: none;
    cursor: pointer;
  }

  .accent-custom input::-webkit-color-swatch-wrapper {
    padding: 0;
  }

  .accent-custom input::-webkit-color-swatch {
    border: none;
    border-radius: 50%;
  }

  .accent-custom span {
    font-family: var(--font-mono);
  }

  :global([data-theme="studio"]) .accent-theme,
  :global([data-theme="studio"]) .accent-custom,
  :global([data-theme="studio"]) .accent-swatch {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .accent-theme,
  :global([data-theme="atelier"]) .accent-custom {
    border-radius: 2px;
  }
</style>
