<!-- Jam (#25) en accès rapide : fenêtre ouverte depuis « En lecture » (bouton `.jam-open`),
     même contenu que Réglages › Application (JamPanel). Ouverte par `openJamDialog`. -->
<script lang="ts">
  import { closeJamDialog, jam } from "../lib/stores/jam.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import JamPanel from "./JamPanel.svelte";

  let dialog = $state<HTMLDivElement>();

  $effect(() => {
    if (jam.dialogOpen) requestAnimationFrame(() => dialog?.focus());
  });

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      closeJamDialog();
    }
  }
</script>

{#if jam.dialogOpen}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="jam-backdrop" onclick={(e) => e.target === e.currentTarget && closeJamDialog()}>
    <div
      class="jam-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="jam-dialog-title"
      tabindex="-1"
      onkeydown={onKeydown}
      bind:this={dialog}
    >
      <header>
        <div class="heading">
          <h3 id="jam-dialog-title" class="display">{t.jam.title}</h3>
          <p class="sub">{t.jam.hint}</p>
        </div>
        <button class="close" onclick={closeJamDialog} aria-label={t.common.close}>
          <Icon name="close" />
        </button>
      </header>
      <div class="body">
        <JamPanel />
      </div>
    </div>
  </div>
{/if}

<style>
  .jam-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.45);
  }

  .jam-dialog {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(560px, 100%);
    max-height: min(720px, 100%);
    padding: 18px;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
  }

  .jam-dialog:focus {
    outline: none;
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  h3 {
    margin: 0;
    font-size: 20px;
  }

  .sub {
    margin: 4px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .close {
    display: grid;
    place-items: center;
    padding: 6px;
    border-radius: 999px;
    color: var(--text-muted);
  }

  .close:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .body {
    overflow-y: auto;
    min-height: 0;
  }

  /* Téléphone : la fenêtre prend la largeur, au-dessus des barres du système. */
  :global(:root[data-platform="android"]) .jam-backdrop {
    padding: calc(var(--safe-top, 0px) + 12px) 12px calc(var(--safe-bottom, 0px) + 12px);
  }
</style>
