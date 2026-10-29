<script lang="ts">
  import { setTheme, THEMES, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import type { Theme } from "../lib/ipc";

  let { onClose }: { onClose: () => void } = $props();

  async function choose(theme: Theme) {
    try {
      await setTheme(theme);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onClose}>
  <div
    class="dialog"
    role="dialog"
    tabindex="-1"
    aria-modal="true"
    aria-label="Apparence"
    onclick={(e) => e.stopPropagation()}
  >
    <h2 class="display">Apparence</h2>
    <p class="hint">Le thème s'applique tout de suite et reste enregistré.</p>
    <div class="themes">
      {#each THEMES as theme (theme.id)}
        <button
          class="theme"
          class:selected={ui.settings.theme === theme.id}
          onclick={() => choose(theme.id)}
          aria-pressed={ui.settings.theme === theme.id}
        >
          <span class="preview {theme.id}" aria-hidden="true">
            <span class="side"></span>
            <span class="main"><span class="h"></span><span class="l"></span><span class="l short"></span></span>
            <span class="bar"></span>
          </span>
          <span class="name">{theme.name}</span>
          <span class="desc">{theme.description}</span>
        </button>
      {/each}
    </div>
    <button class="done" onclick={onClose}>Fermer</button>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.5);
  }

  .dialog {
    width: min(760px, 92vw);
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: calc(var(--panel-radius) + 2px);
    color: var(--text);
    background: var(--surface);
    backdrop-filter: var(--panel-blur);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.4);
  }

  :global([data-theme="studio"]) .dialog,
  :global([data-theme="atelier"]) .dialog {
    background: var(--bg);
  }

  h2 {
    margin: 0;
    font-size: 22px;
  }

  .hint {
    margin: 4px 0 18px;
    color: var(--text-muted);
  }

  .themes {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 14px;
  }

  .theme {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    text-align: left;
  }

  .theme:hover {
    background: var(--bg-hover);
  }

  .theme.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent);
  }

  .name {
    font-weight: 600;
  }

  .desc {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Miniatures dessinées en CSS, indépendantes du thème courant. */
  .preview {
    display: grid;
    grid-template-columns: 22% 1fr;
    grid-template-rows: 1fr 18%;
    gap: 4px;
    height: 110px;
    padding: 6px;
    border-radius: 6px;
    overflow: hidden;
  }

  .side {
    grid-row: 1;
    border-radius: 4px;
  }

  .main {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 8px;
    border-radius: 4px;
  }

  .h {
    width: 60%;
    height: 9px;
    border-radius: 2px;
  }

  .l {
    width: 85%;
    height: 5px;
    border-radius: 2px;
    opacity: 0.6;
  }

  .l.short {
    width: 50%;
  }

  .bar {
    grid-column: 1 / -1;
    border-radius: 4px;
  }

  .preview.seaglass {
    background:
      radial-gradient(circle at 20% 20%, #4f95b0, transparent 60%),
      radial-gradient(circle at 80% 90%, #2c5c6b, transparent 60%),
      #061012;
  }

  .seaglass .side,
  .seaglass .main,
  .seaglass .bar {
    background: rgb(255 255 255 / 0.12);
    border: 1px solid rgb(255 255 255 / 0.12);
  }

  .seaglass .h {
    background: #e9fffa;
  }

  .seaglass .l {
    background: #b8d8d0;
  }

  .preview.studio {
    gap: 1px;
    padding: 0;
    border-radius: 0;
    background: #26262b;
  }

  .studio .side,
  .studio .main,
  .studio .bar {
    border-radius: 0;
    background: #0f0f11;
  }

  .studio .h {
    background: #ffb547;
  }

  .studio .l {
    background: #85858f;
  }

  .preview.atelier {
    gap: 0;
    padding: 0;
    background: #f3efe6;
    border: 1px solid #dcd3c1;
  }

  .atelier .side {
    border-right: 1px solid #dcd3c1;
    border-radius: 0;
  }

  .atelier .bar {
    border-top: 1px solid #1c1a16;
    border-radius: 0;
  }

  .atelier .h {
    height: 12px;
    background: #1c1a16;
  }

  .atelier .l {
    background: #6d675b;
  }

  .done {
    margin-top: 18px;
    padding: 6px 16px;
    border: 1px solid var(--border);
    border-radius: 999px;
  }

  .done:hover {
    background: var(--bg-hover);
  }
</style>
