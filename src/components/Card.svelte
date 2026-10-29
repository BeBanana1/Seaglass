<script lang="ts">
  import type { MediaItem } from "../lib/ipc";
  import { mediaImage, mediaSubtitle, mediaTitle } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";

  let { item }: { item: MediaItem } = $props();

  function open() {
    if (item.kind === "album") go({ name: "album", id: item.id });
    else if (item.kind === "artist") go({ name: "artist", id: item.id });
    else go({ name: "playlist", uuid: item.uuid });
  }

  const image = $derived(mediaImage(item));
</script>

<button class="card" onclick={open}>
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

  .card:hover {
    background: var(--bg-hover);
  }

  .image {
    aspect-ratio: 1;
    margin-bottom: 6px;
    border-radius: var(--radius);
    overflow: hidden;
    background: var(--bg-raised);
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
