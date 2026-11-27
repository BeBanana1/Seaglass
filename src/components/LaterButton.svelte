<script lang="ts">
  // "Later" in a page header (album, #227). The state comes from the backend
  // (lib/stores/later.svelte.ts) and changes once confirmed, like the neighboring heart.
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
  /* Same shape as the heart (FavoriteButton): the themes style `.info .actions button`. */
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
