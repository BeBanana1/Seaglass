<script lang="ts">
  import { dismissToast, toasts } from "../lib/stores/toasts.svelte";
</script>

<div class="stack" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <button class="toast" class:error={toast.level === "error"} onclick={() => dismissToast(toast.id)}>
      {toast.message}
    </button>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: 20px;
    bottom: calc(var(--player-height) + 16px);
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 360px;
  }

  .toast {
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-hover);
    text-align: left;
  }

  .toast.error {
    border-color: var(--danger);
  }
</style>
