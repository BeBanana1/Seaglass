<script lang="ts">
  import type { MediaItem } from "../lib/ipc";
  import { mediaImage, mediaSubtitle, mediaTitle } from "../lib/format";
  import { cardMenu, cardRoute } from "../lib/cardMenu";
  import { menu, menuPosition, openMenu } from "../lib/stores/menu.svelte";
  import { go } from "../lib/stores/nav.svelte";

  let { item }: { item: MediaItem } = $props();

  let cardEl = $state<HTMLButtonElement>();
  const menuOpen = $derived(!!cardEl && menu.open?.owner === cardEl);

  // Clic droit, touche Menu ou Maj+F10 (#81) : ouvrir, favori, liens.
  function onContextmenu(e: MouseEvent) {
    e.preventDefault();
    const card = e.currentTarget as HTMLButtonElement;
    openMenu({ ...menuPosition(e, card), entries: cardMenu(item), from: card, owner: card });
  }

  const image = $derived(mediaImage(item));
</script>

<button
  class="card"
  class:menu-open={menuOpen}
  bind:this={cardEl}
  onclick={() => go(cardRoute(item))}
  oncontextmenu={onContextmenu}
>
  <div class="image" class:round={item.kind === "artist"}>
    {#if image}
      <img src={image} alt="" loading="lazy" />
    {/if}
  </div>
  <span class="title">
    {mediaTitle(item)}
    {#if item.kind === "album" && item.hires}<span class="badge">HI-RES</span>{/if}
  </span>
  <span class="sub">{mediaSubtitle(item)}</span>
</button>

<style>
  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    padding: 10px;
    border-radius: var(--radius);
    text-align: left;
  }

  .card:hover,
  .card.menu-open {
    background: var(--bg-hover);
  }

  .image {
    aspect-ratio: 1;
    margin-bottom: 6px;
    border-radius: var(--radius-card);
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow: var(--shadow-cover, none);
  }

  .image.round {
    border-radius: 50%;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .title,
  .sub {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
  }

  .title {
    font-weight: 500;
  }

  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
