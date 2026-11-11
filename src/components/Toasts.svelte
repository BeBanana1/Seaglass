<script lang="ts">
  import { dismissToast, toasts } from "../lib/stores/toasts.svelte";
</script>

<div class="stack" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <button
      class="toast"
      class:error={toast.level === "error"}
      onclick={() => {
        toast.action?.();
        dismissToast(toast.id);
      }}
    >
      {toast.message}
      {#if toast.actionLabel}<span class="toast-action">{toast.actionLabel}</span>{/if}
    </button>
  {/each}
</div>

<style>
  /* Centrée juste au-dessus de la barre de lecture (#150), au-dessus de « En lecture »
     (z-index 50) et de la mise à jour (55), sous le menu contextuel (100). */
  .stack {
    position: fixed;
    left: 50%;
    bottom: calc(var(--player-height) + var(--shell-gap) + 12px);
    z-index: 60;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: min(420px, calc(100vw - 40px));
    transform: translateX(-50%);
    pointer-events: none;
  }

  /* Au-dessus du mini-lecteur et de la navigation du bas (comme la mise à jour), et du
     clavier quand il est ouvert : `--keyboard-inset` vient du plugin Android (#251). */
  :global(:root[data-platform="android"]) .stack {
    bottom: max(calc(var(--safe-bottom) + 140px), calc(var(--keyboard-inset, 0px) + 12px));
    width: calc(100vw - 24px);
  }

  /* « En lecture » couvre la barre de lecture : en bas de l'écran, pour ne pas
     recouvrir la progression ni les commandes. */
  :global(:root:has(.now-playing)) .stack {
    bottom: max(calc(var(--safe-bottom) + 20px), calc(var(--keyboard-inset, 0px) + 12px));
  }

  /* Surface pleine du menu contextuel : lisible sur n'importe quel contenu, dans chaque thème. */
  .toast {
    max-width: 100%;
    padding: 10px 16px;
    border: var(--dialog-border);
    border-radius: var(--popover-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--popover-shadow);
    font-size: 13px;
    text-align: center;
    pointer-events: auto;
    animation: toast-in 0.18s ease-out;
  }

  .toast.error {
    border: 1px solid var(--danger);
  }

  /* Action du toast (« Annuler », #349) : tout le toast est cliquable, le libellé le dit. */
  .toast-action {
    margin-left: 10px;
    color: var(--accent);
    font-weight: 600;
    text-decoration: underline;
    text-underline-offset: 3px;
  }

  @keyframes toast-in {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }
</style>
