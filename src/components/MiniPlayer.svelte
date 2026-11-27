<script lang="ts">
  // Android mini-player (#24), above the bottom navigation: a tap opens
  // "Now playing" full screen, where the other controls are.
  // No progress bar (#473, #480): a moving bar forces the WebView to recompose
  // the page, 4 times per second with a timer (24% of a core during playback on the S21),
  // on every screen refresh with a compositor animation (98%). The
  // position is read in "Now playing", as in the TIDAL app.
  import { commands } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { coverSmall, fullTitle } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import LikeButton from "./LikeButton.svelte";
  import { open } from "../lib/actions";

  let { onOpenNowPlaying }: { onOpenNowPlaying: () => void } = $props();

  const track = $derived(player.currentTrack);
  const playing = $derived(player.playing);
  const s = $derived(player.state);
</script>

{#if track}
  <div class="mini-player panel">
    <button class="mini-open" onclick={onOpenNowPlaying} aria-label={t.player.openNowPlaying}>
      <span class="mini-cover">
        {#if track.album.coverUrl}<img src={coverSmall(track.album)} alt="" />{/if}
      </span>
      <span class="mini-meta">
        <span class="mini-title">{fullTitle(track)}</span>
        <span class="mini-artist">{track.artists.map((a) => a.name).join(", ")}</span>
      </span>
    </button>
    <LikeButton {track} size={20} />
    <button
      class="mini-play"
      onclick={() => commands.togglePause()}
      aria-label={playing ? t.player.pause : t.player.play}
    >
      {#if s.loading || s.buffering}
        <span class="spinner"></span>
      {:else}
        <Icon name={playing ? "pause" : "play"} size={24} />
      {/if}
    </button>
    <button
      class="mini-next"
      onclick={() => commands.next()}
      disabled={!s.hasNext}
      aria-label={t.player.next}
    >
      <Icon name="next" size={20} />
    </button>
  </div>
{:else if s.blindTest}
  <!-- Blind test (#434): title hidden, a tap goes back to the game. -->
  <div class="mini-player panel">
    <button class="mini-open" onclick={() => open({ name: "blindTest" })}>
      <span class="mini-cover blind"><Icon name="quiz" size={22} /></span>
      <span class="mini-meta"><span class="mini-title">{t.blindTest.inProgress}</span></span>
    </button>
  </div>
{/if}

<style>
  .mini-cover.blind {
    display: grid;
    place-items: center;
    color: var(--accent);
  }

  .mini-player {
    position: relative;
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 6px 6px 6px 6px;
    overflow: hidden;
  }

  .mini-open {
    display: flex;
    flex: 1;
    align-items: center;
    gap: 10px;
    min-width: 0;
    padding: 0;
    text-align: left;
  }

  .mini-cover {
    flex-shrink: 0;
    width: 44px;
    height: 44px;
    border-radius: var(--radius-cover, 6px);
    overflow: hidden;
    background: var(--bg-raised);
  }

  .mini-cover img {
    display: block;
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .mini-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .mini-title,
  .mini-artist {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .mini-title {
    font-weight: 600;
  }

  .mini-artist {
    font-size: 12px;
    color: var(--text-muted);
  }

  .mini-play,
  .mini-next {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 44px;
    height: 44px;
  }

  .mini-next:disabled {
    opacity: 0.35;
  }

  .spinner {
    width: 20px;
    height: 20px;
    border: 2px solid var(--border);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
