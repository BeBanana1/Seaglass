<script lang="ts">
  // Favorite of an album, a playlist or a mix (heart), "Follow" for an artist (#81, #204).
  // The state comes from the backend (lib/stores/likes.svelte.ts), changes once confirmed.
  import type { LikeTarget } from "../lib/ipc";
  import { isFavorite, isPending, toggleFavorite } from "../lib/stores/likes.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let { target }: { target: LikeTarget } = $props();

  const liked = $derived(isFavorite(target));
  const pending = $derived(isPending(target));
  const follow = $derived(target.kind === "artist");
  const label = $derived(
    follow
      ? liked
        ? t.menu.unfollow
        : t.menu.follow
      : liked
        ? t.menu.removeFromLibrary
        : t.menu.addToLibrary,
  );
</script>

<button
  class="favorite-button"
  class:liked
  class:pending
  class:follow
  onclick={() => toggleFavorite(target)}
  title={label}
  aria-label={follow ? undefined : label}
  aria-pressed={liked}
>
  {#if follow}
    {liked ? t.menu.following : t.menu.follow}
  {:else}
    <Icon name={liked ? "heartFilled" : "heart"} size={18} />
  {/if}
</button>

<style>
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

  button.liked {
    color: var(--accent);
  }

  button.follow {
    width: auto;
    padding: 0 18px;
    font-weight: 600;
    color: var(--text);
  }

  button.follow.liked {
    border-color: var(--accent);
    color: var(--accent);
  }

  button.pending {
    opacity: 0.5;
    cursor: progress;
  }
</style>
