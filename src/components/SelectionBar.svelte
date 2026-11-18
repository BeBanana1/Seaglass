<script lang="ts">
  // Barre d'actions de la sélection multiple (#332). Android : à la place du mini-lecteur ;
  // bureau : flottante au-dessus de la barre de lecture.
  import { selection } from "../lib/stores/selection.svelte";
  import { likes } from "../lib/stores/likes.svelte";
  import {
    addSelectionToPlaylist,
    allSelectedLiked,
    canRemoveSelection,
    enqueueSelection,
    likeSelection,
    removeSelection,
  } from "../lib/selectionActions";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import Icon from "./Icon.svelte";

  const m = $derived(t.menu);
  // Relu quand la sélection ou les favoris changent.
  const liked = $derived.by(() => {
    void selection.count;
    void likes.tracks;
    return allSelectedLiked();
  });
  const size = $derived(platform.mobile ? 22 : 18);
</script>

<div
  class="selection-bar panel"
  class:mobile={platform.mobile}
  role="toolbar"
  aria-label={m.selection.count(selection.count)}
>
  <button
    class="selection-clear"
    onclick={() => selection.clear()}
    title={m.selection.clear}
    aria-label={m.selection.clear}
  >
    <Icon name="close" {size} />
  </button>
  <!-- Android : libellé court, la barre a la largeur du téléphone. -->
  <span class="selection-count">
    {platform.mobile ? m.selection.label(selection.count) : m.selection.count(selection.count)}
  </span>
  <div class="selection-actions">
    <button
      onclick={() => selection.selectAll()}
      title={m.selection.selectAll}
      aria-label={m.selection.selectAll}
    >
      <Icon name="check" {size} />
    </button>
    <button onclick={() => enqueueSelection(true)} title={m.playNext} aria-label={m.playNext}>
      <Icon name="playNext" {size} />
    </button>
    <button onclick={() => enqueueSelection(false)} title={m.addToQueue} aria-label={m.addToQueue}>
      <Icon name="add" {size} />
    </button>
    <button onclick={addSelectionToPlaylist} title={m.addToPlaylist} aria-label={m.addToPlaylist}>
      <Icon name="playlistAdd" {size} />
    </button>
    <button
      onclick={likeSelection}
      class:liked
      title={liked ? m.selection.unlikeAll : m.selection.likeAll}
      aria-label={liked ? m.selection.unlikeAll : m.selection.likeAll}
    >
      <Icon name={liked ? "heartFilled" : "heart"} {size} />
    </button>
    {#if canRemoveSelection()}
      <button
        onclick={removeSelection}
        title={m.removeFromPlaylist}
        aria-label={m.removeFromPlaylist}
      >
        <Icon name="remove" {size} />
      </button>
    {/if}
  </div>
</div>

<style>
  .selection-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 8px;
  }

  /* Bureau : au-dessus de la barre de lecture, centrée sur la fenêtre. */
  .selection-bar:not(.mobile) {
    position: fixed;
    z-index: 20;
    left: 50%;
    bottom: calc(var(--player-height) + 2 * var(--shell-gap) + 12px);
    translate: -50% 0;
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }

  .selection-count {
    flex: 1;
    padding: 0 6px;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
  }

  .selection-actions {
    display: flex;
    gap: 2px;
  }

  button {
    display: grid;
    place-items: center;
    padding: 6px;
    border-radius: 6px;
    color: var(--text-muted);
  }

  button:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  button.liked {
    color: var(--accent);
  }

  /* Android : même hauteur que le mini-lecteur qu'elle remplace, cibles au doigt. */
  .selection-bar.mobile {
    min-height: 56px;
    gap: 2px;
  }

  .selection-bar.mobile button {
    padding: 8px;
  }

  .selection-bar.mobile .selection-count {
    padding: 0 2px;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 14px;
  }
</style>
