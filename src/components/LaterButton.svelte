<script lang="ts">
  // « Plus tard » dans l'en-tête d'une page (album, #227). L'état vient du backend
  // (lib/stores/later.svelte.ts) et change à sa confirmation, comme le cœur voisin.
  import type { LikeTarget } from "../lib/ipc";
  import { isLaterPending, isSavedForLater, toggleLater } from "../lib/stores/later.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let { target }: { target: LikeTarget } = $props();

  const saved = $derived(isSavedForLater(target));
  const pending = $derived(isLaterPending(target));
  const label = $derived(saved ? t.menu.removeFromLater : t.menu.saveForLater);
</script>

<button
  class="later-button"
  class:saved
  class:pending
  onclick={() => toggleLater(target)}
  title={label}
  aria-label={label}
  aria-pressed={saved}
>
  <Icon name={saved ? "bookmarkFilled" : "bookmark"} size={18} />
</button>

<style>
  /* Même forme que le cœur (FavoriteButton) : les thèmes habillent `.info .actions button`. */
  button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 37px;
    height: 37px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--text-muted);
  }

  button:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  button.saved {
    color: var(--accent);
  }

  button.pending {
    opacity: 0.5;
    cursor: progress;
  }
</style>
