<script lang="ts">
  // Mini-lecteur d'Android (#24), au-dessus de la navigation du bas : un appui ouvre
  // « En lecture » en plein écran, où sont les autres commandes.
  import { commands } from "../lib/ipc";
  import { animateProgress, player } from "../lib/stores/player.svelte";
  import { coverSmall, fullTitle } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";
  import LikeButton from "./LikeButton.svelte";
  import { open } from "../lib/actions";

  let { onOpenNowPlaying }: { onOpenNowPlaying: () => void } = $props();

  const track = $derived(player.currentTrack);
  const playing = $derived(player.playing);
  const s = $derived(player.state);

  let bar = $state<HTMLElement>();
  // Barre animée par le compositeur (#473), reprise à chaque état reçu : ni minuteur ni
  // recalcul de style pendant la lecture.
  $effect(() => {
    if (bar) return animateProgress(bar);
  });
</script>

{#if track}
  <div class="mini-player panel">
    <div class="mini-progress" bind:this={bar}></div>
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
    <button class="mini-next" onclick={() => commands.next()} disabled={!s.hasNext} aria-label={t.player.next}>
      <Icon name="next" size={20} />
    </button>
  </div>
{:else if s.blindTest}
  <!-- Blind test (#434) : titre masqué, un appui ramène à la partie. -->
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

  /* Fine barre en haut du mini-lecteur ; le saut se fait dans « En lecture ». */
  .mini-progress {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 2px;
    background: var(--accent);
    transform: scaleX(0);
    transform-origin: left;
    will-change: transform;
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
