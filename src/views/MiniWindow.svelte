<script lang="ts">
  // Desktop mini player (#384): the only view of the `mini` window, in place of the app.
  // It shows the player state and sends its commands, nothing else; Rust opens and
  // closes the window, and shows the main window again when it closes.
  import { onMount } from "svelte";
  import { commands } from "../lib/ipc";
  import { followPosition, initPlayer, player } from "../lib/stores/player.svelte";
  import { initLikes, loadLikes } from "../lib/stores/likes.svelte";
  import { initUi } from "../lib/stores/ui.svelte";
  import { initLanguage, t } from "../lib/i18n.svelte";
  import { coverSmall, fullTitle } from "../lib/format";
  import Icon from "../components/Icon.svelte";
  import LikeButton from "../components/LikeButton.svelte";

  let onTop = $state(true);
  let ratio = $state(0);

  const track = $derived(player.currentTrack);
  const s = $derived(player.state);
  const playing = $derived(!s.paused && !s.idle);

  onMount(async () => {
    await Promise.all([initLanguage(), initUi(), initPlayer(), initLikes()]);
    void loadLikes();
    onTop = await commands.miniPlayerOnTop().catch(() => true);
  });

  // Color of the current cover, picked up by the themes (`--album`), as in the app.
  $effect(() => {
    const color = player.currentTrack?.album.color;
    if (color) document.documentElement.style.setProperty("--album", color);
  });

  // Boucle d'images seulement pendant la lecture (#88).
  $effect(() => {
    const duration = player.state.duration;
    return followPosition((p) => (ratio = duration ? Math.min(p / duration, 1) : 0));
  });

  function back() {
    void commands.closeMiniPlayer();
  }

  async function toggleOnTop() {
    onTop = await commands.setMiniPlayerOnTop(!onTop).catch(() => onTop);
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape" || (e.ctrlKey && e.key.toLowerCase() === "m")) {
      e.preventDefault();
      back();
    } else if (e.key === " " && !(e.target as HTMLElement).closest("button")) {
      e.preventDefault();
      void commands.togglePause();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} oncontextmenu={(e) => e.preventDefault()} />

<!-- Borderless window: it is moved by dragging the background or the text. -->
<div class="mini" data-tauri-drag-region>
  <button class="cover" ondblclick={back} title={t.player.miniBack}>
    {#if track?.album.coverUrl}<img src={coverSmall(track.album)} alt="" />{/if}
  </button>

  <div class="body" data-tauri-drag-region>
    <div class="top" data-tauri-drag-region>
      <div class="meta" data-tauri-drag-region>
        {#if track}
          <span class="title" data-tauri-drag-region>{fullTitle(track)}</span>
          <span class="artist" data-tauri-drag-region>
            {track.artists.map((a) => a.name).join(", ")}
          </span>
        {:else}
          <span class="artist" data-tauri-drag-region>{t.player.nothingPlaying}</span>
        {/if}
      </div>
      <button
        class="tool"
        class:on={onTop}
        onclick={toggleOnTop}
        title={t.player.miniOnTop}
        aria-pressed={onTop}
      >
        <Icon name="pin" size={15} />
      </button>
      <button class="tool" onclick={back} title={t.player.miniBack}>
        <Icon name="fullscreen" size={15} />
      </button>
    </div>

    <div class="controls" data-tauri-drag-region>
      <button onclick={() => commands.previous()} disabled={!s.track} title={t.player.previous}>
        <Icon name="previous" size={18} />
      </button>
      <button
        class="play"
        onclick={() => commands.togglePause()}
        disabled={!s.track}
        aria-label={playing ? t.player.pause : t.player.play}
        title={playing ? t.player.pauseHint : t.player.playHint}
      >
        <Icon name={playing ? "pause" : "play"} size={18} />
      </button>
      <button onclick={() => commands.next()} disabled={!s.hasNext} title={t.player.next}>
        <Icon name="next" size={18} />
      </button>
      {#if track}<span class="like"><LikeButton {track} size={17} /></span>{/if}
    </div>
  </div>

  <div class="progress" style:--ratio={ratio}></div>
</div>

<style>
  :global(html),
  :global(body) {
    overflow: hidden;
  }

  .mini {
    position: fixed;
    inset: 0;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px;
    color: var(--text);
    background: var(--dialog-bg, var(--bg));
    border: 1px solid var(--border);
    user-select: none;
  }

  .cover {
    flex-shrink: 0;
    width: 86px;
    height: 86px;
    padding: 0;
    border-radius: var(--radius-card, 6px);
    overflow: hidden;
    background: var(--bg-raised);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .body {
    display: flex;
    flex: 1;
    flex-direction: column;
    justify-content: space-between;
    align-self: stretch;
    min-width: 0;
  }

  .top {
    display: flex;
    align-items: flex-start;
    gap: 2px;
  }

  .meta {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .title,
  .artist {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .artist {
    font-size: 12px;
    color: var(--text-muted);
  }

  button {
    display: grid;
    place-items: center;
    color: var(--text-muted);
  }

  button:hover:not(:disabled) {
    color: var(--text);
  }

  button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .tool {
    padding: 4px;
  }

  .tool.on {
    color: var(--accent);
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .controls > button {
    padding: 4px;
  }

  .play {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .play:hover:not(:disabled) {
    color: var(--accent-contrast);
    background: var(--accent-strong);
  }

  .like {
    margin-left: auto;
  }

  .progress {
    position: absolute;
    right: 0;
    bottom: 0;
    left: 0;
    height: 3px;
    background: linear-gradient(
      to right,
      var(--accent) calc(var(--ratio) * 100%),
      transparent calc(var(--ratio) * 100%)
    );
  }
</style>
