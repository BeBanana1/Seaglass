<script lang="ts">
  import type { Track } from "../lib/ipc";
  import { artistNames, formatTime, fullTitle } from "../lib/format";

  let {
    track,
    active = false,
    onPlay,
  }: { track: Track; active?: boolean; onPlay: () => void } = $props();
</script>

<button class="row" class:active ondblclick={onPlay} onkeydown={(e) => e.key === "Enter" && onPlay()}>
  {#if track.album.coverUrl}
    <img src={track.album.coverUrl} alt="" loading="lazy" />
  {:else}
    <div class="cover-placeholder"></div>
  {/if}
  <div class="main">
    <span class="title">
      {fullTitle(track)}
      {#if track.explicit}<span class="badge muted">E</span>{/if}
      {#if track.hires}<span class="badge">HI-RES</span>{/if}
    </span>
    <span class="sub">{artistNames(track)}</span>
  </div>
  <span class="album">{track.album.title}</span>
  <span class="duration">{formatTime(track.duration)}</span>
</button>

<style>
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 2fr) minmax(0, 1fr) 56px;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 6px 10px;
    border-radius: var(--radius);
    text-align: left;
  }

  .row:hover {
    background: var(--bg-hover);
  }

  .row.active .title {
    color: var(--accent);
  }

  img,
  .cover-placeholder {
    width: 40px;
    height: 40px;
    border-radius: 4px;
    object-fit: cover;
    background: var(--bg-raised);
  }

  .main {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title,
  .sub,
  .album {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .sub,
  .album,
  .duration {
    color: var(--text-muted);
  }

  .duration {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
