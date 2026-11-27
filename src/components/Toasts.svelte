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
  /* Centered just above the player bar (#150), above "Now playing"
     (z-index 50) and the update (55), below the context menu (100). */
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

  /* Above the mini-player and the bottom navigation (like the update), and the
     keyboard when it is open: `--keyboard-inset` comes from the Android plugin (#251). */
  :global(:root[data-platform="android"]) .stack {
    bottom: max(calc(var(--safe-bottom) + 140px), calc(var(--keyboard-inset, 0px) + 12px));
    width: calc(100vw - 24px);
  }

  /* "Now playing" covers the player bar: at the bottom of the screen, so as not to
     cover the progress or the controls. */
  :global(:root:has(.now-playing)) .stack {
    bottom: max(calc(var(--safe-bottom) + 20px), calc(var(--keyboard-inset, 0px) + 12px));
  }

  /* Solid surface of the context menu: readable over any content, in every theme. */
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

  /* Toast action ("Undo", #349): the whole toast is clickable, the label says so. */
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
