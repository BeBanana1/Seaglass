<!-- Choix du thème (section Apparence des réglages), avec une miniature de chacun. -->
<script lang="ts">
  import { setTheme, THEMES, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import type { Theme } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";

  async function choose(theme: Theme) {
    try {
      await setTheme(theme);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<div class="themes" role="radiogroup" aria-label={t.settings.theme}>
  {#each THEMES as id (id)}
    {@const theme = { id, ...t.settings.themes[id] }}
    <button
      class="theme-card"
      class:selected={ui.settings.theme === theme.id}
      onclick={() => choose(theme.id)}
      role="radio"
      aria-checked={ui.settings.theme === theme.id}
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

<style>
  .themes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    gap: 14px;
  }

  .theme-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 10px;
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

  .preview.acidwave {
    gap: 3px;
    background:
      radial-gradient(circle at 15% 15%, rgb(200 255 0 / 0.55), transparent 50%),
      radial-gradient(circle at 85% 70%, rgb(255 43 214 / 0.55), transparent 50%),
      #050507;
  }

  .acidwave .side,
  .acidwave .main,
  .acidwave .bar {
    border-radius: 2px;
    background: rgb(8 8 12 / 0.75);
    border: 1px solid rgb(200 255 0 / 0.35);
  }

  .acidwave .h {
    background: linear-gradient(180deg, #fff, #4d5560 50%, #eef3f7);
    box-shadow: 0 0 6px rgb(200 255 0 / 0.6);
  }

  .acidwave .l {
    background: #c8ff00;
  }

  .preview.surreal {
    position: relative;
    gap: 5px;
    padding: 8px;
    border-radius: 12px;
    background:
      radial-gradient(circle at 72% 62%, #f6b38a 0 16%, transparent 17%),
      linear-gradient(180deg, #e8d9f0, #fbe7d8 70%);
  }

  .surreal .side,
  .surreal .main,
  .surreal .bar {
    border-radius: 10px;
    background: rgb(255 250 244 / 0.8);
    box-shadow: 0 6px 10px -6px rgb(61 42 77 / 0.5);
  }

  .surreal .h {
    background: #3d2a4d;
    border-radius: 6px;
  }

  .surreal .l {
    background: #b49cbf;
  }

  .preview.solarpunk {
    gap: 5px;
    padding: 8px;
    border-radius: 12px;
    background:
      radial-gradient(circle at 82% 18%, #f3cf6b 0 14%, transparent 15%),
      radial-gradient(ellipse at 0% 100%, #2e6b3d 0 26%, transparent 27%),
      radial-gradient(ellipse at 100% 100%, #3f8a4f 0 22%, transparent 23%),
      linear-gradient(180deg, #f8efcc, #e6eecf 75%);
  }

  .solarpunk .side,
  .solarpunk .main,
  .solarpunk .bar {
    border-radius: 9px;
    background: rgb(251 249 236 / 0.82);
    box-shadow: 0 6px 10px -6px rgb(24 64 36 / 0.45);
  }

  .solarpunk .main {
    border-radius: 18px 18px 9px 9px;
  }

  .solarpunk .h {
    background: #15301d;
    border-radius: 6px;
  }

  .solarpunk .l {
    background: #2c7a41;
  }

  :global([data-theme="studio"]) .theme-card {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .theme-card {
    border-radius: 2px;
  }
</style>
