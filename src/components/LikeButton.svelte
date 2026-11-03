<script lang="ts">
  // Cœur d'un titre (#4) : l'état vient du backend (lib/stores/likes.svelte.ts).
  import type { Track } from "../lib/ipc";
  import { isLiked, isPending, toggleLike } from "../lib/stores/likes.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let { track, size = 16 }: { track: Track; size?: number } = $props();

  const liked = $derived(isLiked(track.id));
  const pending = $derived(isPending({ kind: "track", id: track.id }));
</script>

<button
  class="like-button"
  class:liked
  class:pending
  onclick={(e) => {
    e.stopPropagation();
    toggleLike(track);
  }}
  ondblclick={(e) => e.stopPropagation()}
  aria-pressed={liked}
  title={liked ? t.menu.unlike : t.menu.like}
  aria-label={liked ? t.menu.unlike : t.menu.like}
>
  <Icon name={liked ? "heartFilled" : "heart"} {size} />
</button>

<style>
  .like-button {
    display: grid;
    place-items: center;
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .like-button:hover {
    color: var(--text);
  }

  .like-button.liked {
    color: var(--accent);
  }

  .like-button.pending {
    opacity: 0.5;
    cursor: progress;
  }
</style>
