<script lang="ts">
  // Vue immersive « En lecture » : grande pochette, chaîne audio, à suivre,
  // et panneau latéral repliable : file d'attente (#20) ou paroles (#5).
  import { untrack } from "svelte";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { ui, type NowPlayingPanel } from "../lib/stores/ui.svelte";
  import { fullTitle } from "../lib/format";
  import { go, nav } from "../lib/stores/nav.svelte";
  import { openPlaylistPicker } from "../lib/stores/playlistPicker.svelte";
  import { openTrackInfo } from "../lib/stores/trackInfo.svelte";
  import Backdrop from "../components/backdrops/Backdrop.svelte";
  import Icon from "../components/Icon.svelte";
  import LikeButton from "../components/LikeButton.svelte";
  import Lyrics from "../components/Lyrics.svelte";
  import Progress from "../components/Progress.svelte";
  import QueueList from "../components/QueueList.svelte";
  import SignalPath from "../components/SignalPath.svelte";
  import Transport from "../components/Transport.svelte";
  import Volume from "../components/Volume.svelte";
  import { t } from "../lib/i18n.svelte";

  let { onClose }: { onClose: () => void } = $props();

  const s = $derived(player.state);
  const upNext = $derived([...queue.view.manual, ...queue.view.upcoming].slice(0, 5));
  const panel = $derived(ui.nowPlayingPanel);
  const queueOpen = $derived(panel === "queue");

  function toggle(p: Exclude<NowPlayingPanel, null>) {
    ui.nowPlayingPanel = ui.nowPlayingPanel === p ? null : p;
  }

  // Toute navigation (lien d'artiste ou d'album, ici ou dans la file) ferme la vue,
  // sinon la page ouverte resterait cachée derrière.
  const stackAtOpen = untrack(() => nav.stack);
  $effect(() => {
    if (nav.stack !== stackAtOpen) onClose();
  });
</script>

<div class="now-playing" class:lyrics-open={panel === "lyrics"} role="dialog" aria-modal="true" aria-label={t.views.nowPlaying.title}>
  <Backdrop context="nowplaying" />
  <div class="top-actions" class:with-panel={panel}>
    <button
      class="lyrics-toggle"
      class:on={panel === "lyrics"}
      onclick={() => toggle("lyrics")}
      aria-pressed={panel === "lyrics"}
      title={t.views.nowPlaying.lyrics}
    >
      <Icon name="lyrics" size={20} />
    </button>
    <button
      class="queue-toggle"
      class:on={queueOpen}
      onclick={() => toggle("queue")}
      aria-pressed={queueOpen}
      title={t.player.queue}
    >
      <Icon name="queue" size={20} />
    </button>
    <button class="close" onclick={onClose} title={t.views.nowPlaying.close}>
      <Icon name="close" size={20} />
    </button>
  </div>

  <div class="scroller" class:with-panel={panel}>
    <div class="stage">
      {#if s.track}
        {@const track = s.track}
        <div class="layout">
          <div class="art">
            {#if track.album.coverUrl}
              <img src={track.album.coverUrl.replace("320x320", "640x640")} alt="" />
            {/if}
          </div>
          <div class="info">
            <span class="label">{t.views.nowPlaying.title}</span>
            <h1 class="display">{fullTitle(track)}</h1>
            <div class="who">
              {#each track.artists as artist, i (artist.id)}
                {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => go({ name: "artist", id: artist.id })}
                  >{artist.name}</button
                >
              {/each}
            </div>
            <button class="link album" onclick={() => go({ name: "album", id: track.album.id })}>{track.album.title}</button>
            <div class="now-actions">
              <LikeButton {track} size={20} />
              <button
                class="now-add"
                onclick={() => openPlaylistPicker({ trackIds: [track.id], label: fullTitle(track) })}
                title={t.views.header.addToPlaylist}
                aria-label={t.views.header.addToPlaylist}
              >
                <Icon name="playlistAdd" size={20} />
              </button>
              <button class="now-add" onclick={() => openTrackInfo(track)} title={t.menu.trackInfo} aria-label={t.menu.trackInfo}>
                <Icon name="info" size={20} />
              </button>
            </div>

            <div class="controls">
              <Progress large />
              <Transport large />
              <Volume />
            </div>

            <div class="signal"><SignalPath /></div>

            {#if upNext.length > 0 && !panel}
              <div class="next">
                <span class="label">{t.views.nowPlaying.upNext}</span>
                <ol>
                  {#each upNext as next, i (`${i}-${next.id}`)}
                    <li>
                      {#if next.album.coverUrl}<img src={next.album.coverUrl} alt="" />{/if}
                      <span class="t">{fullTitle(next)}</span>
                      <span class="a">{next.artists.map((a) => a.name).join(", ")}</span>
                    </li>
                  {/each}
                </ol>
              </div>
            {/if}
          </div>
        </div>
      {:else}
        <p class="empty">{t.views.nowPlaying.empty}</p>
      {/if}
    </div>

    {#if queueOpen}
      <aside class="panel np-panel np-queue" aria-label={t.player.queue}>
        <h2>{t.player.queue}</h2>
        <QueueList current={false} />
      </aside>
    {:else if panel === "lyrics"}
      <aside class="panel np-panel np-lyrics" aria-label={t.views.nowPlaying.lyrics}>
        <h2>{t.views.nowPlaying.lyrics}</h2>
        <Lyrics />
      </aside>
    {/if}
  </div>
</div>

<style>
  .now-playing {
    --np-panel-width: 380px;
    position: fixed;
    inset: 0;
    z-index: 50;
    overflow: hidden;
    background: var(--bg);
    animation: rise 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .now-playing.lyrics-open {
    --np-panel-width: 460px;
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(24px) scale(0.98);
    }
  }

  .top-actions {
    position: absolute;
    top: 18px;
    right: 18px;
    z-index: 2;
    display: flex;
    gap: 8px;
  }

  .close,
  .queue-toggle,
  .lyrics-toggle {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    color: var(--text);
    background: var(--bg-raised);
  }

  .close:hover,
  .queue-toggle:hover,
  .lyrics-toggle:hover {
    background: var(--bg-hover);
  }

  .queue-toggle.on,
  .lyrics-toggle.on {
    color: var(--accent);
  }

  /* Défilement : la vue entière, ou chaque colonne quand la file est à droite. */
  .scroller {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    height: 100%;
    overflow: auto;
  }

  .stage {
    display: grid;
    min-width: 0;
  }

  .layout {
    position: relative;
    display: grid;
    grid-template-columns: minmax(240px, 46vh) minmax(320px, 560px);
    align-items: center;
    gap: clamp(32px, 6vw, 88px);
    margin: auto;
    padding: 48px;
  }

  .art {
    aspect-ratio: 1;
    border-radius: 14px;
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow:
      0 30px 80px rgb(0 0 0 / 0.5),
      0 0 120px color-mix(in oklab, var(--album) 35%, transparent);
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: clamp(28px, 4.2vw, 52px);
    line-height: 1.05;
  }

  .now-actions {
    display: flex;
    gap: 6px;
    margin-left: -4px;
  }

  .now-add {
    display: grid;
    place-items: center;
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .now-add:hover {
    color: var(--text);
  }

  .who {
    font-size: 16px;
    color: var(--text-muted);
  }

  .link {
    color: var(--text);
  }

  .link.album {
    align-self: flex-start;
    text-align: left;
    color: var(--text-muted);
  }

  .link:hover {
    text-decoration: underline;
  }

  .controls {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
    margin: 22px 0 8px;
  }

  .signal {
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-raised);
    backdrop-filter: blur(12px);
  }

  .next {
    margin-top: 10px;
  }

  ol {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }

  li {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) minmax(0, 0.8fr);
    align-items: center;
    gap: 10px;
    padding: 5px 0;
    font-size: 13px;
  }

  li img {
    width: 32px;
    height: 32px;
    border-radius: 3px;
  }

  .t,
  .a {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .a {
    color: var(--text-muted);
  }

  .empty {
    margin: auto;
    color: var(--text-muted);
  }

  /* Panneau latéral : sous la vue par défaut (fenêtre étroite), à droite si la place le permet. */
  .np-panel {
    position: relative;
    justify-self: center;
    width: min(100% - 48px, 420px);
    margin-bottom: 24px;
    padding: 16px 12px;
  }

  .np-lyrics {
    display: flex;
    flex-direction: column;
    height: min(70vh, 640px);
  }

  h2 {
    margin: 0 8px 4px;
    font-size: 15px;
    font-weight: 600;
  }

  @media (max-width: 900px) {
    .layout {
      grid-template-columns: minmax(0, 420px);
      padding: 72px 24px 24px;
    }
  }

  @media (min-width: 901px) {
    .scroller.with-panel {
      grid-template-columns: minmax(0, 1fr) var(--np-panel-width);
      grid-template-rows: minmax(0, 1fr);
      overflow: hidden;
    }

    .with-panel .stage {
      min-height: 0;
      overflow: auto;
    }

    .with-panel .np-panel {
      justify-self: stretch;
      width: auto;
      height: auto;
      margin: 16px 16px 16px 0;
      overflow: auto;
    }

    /* Les paroles défilent dans leur propre zone, sous le titre du panneau. */
    .with-panel .np-lyrics {
      overflow: hidden;
    }

    /* Les boutons restent au-dessus de la pochette, pas sur le panneau. */
    .top-actions.with-panel {
      right: calc(var(--np-panel-width) + 34px);
    }

    .with-panel .layout {
      grid-template-columns: minmax(200px, min(46vh, 32vw)) minmax(320px, 560px);
      gap: clamp(32px, 4vw, 64px);
    }
  }

  /* Entre les deux : file à droite, pochette au-dessus des commandes. */
  @media (min-width: 901px) and (max-width: 1280px) {
    .with-panel .layout {
      grid-template-columns: minmax(0, 420px);
      padding: 72px 32px 32px;
    }
  }

  /* Studio : pas de flou, grille nette, pochette sans ombre colorée. */
  :global([data-theme="studio"]) .art {
    border-radius: 0;
    box-shadow: none;
    outline: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .signal {
    border-radius: 0;
    backdrop-filter: none;
  }

  :global([data-theme="studio"]) .close,
  :global([data-theme="studio"]) .queue-toggle,
  :global([data-theme="studio"]) .lyrics-toggle {
    border-radius: 0;
  }

  :global([data-theme="studio"]) .np-panel,
  :global([data-theme="atelier"]) .np-panel {
    border: 1px solid var(--border);
  }

  /* Atelier : page de magazine, sans fond flouté. */
  :global([data-theme="atelier"]) .art {
    border-radius: 0;
    box-shadow: 18px 18px 0 color-mix(in oklab, var(--album) 35%, var(--bg));
  }

  :global([data-theme="atelier"]) .signal {
    border-radius: 0;
    border-color: var(--text);
    background: transparent;
  }

  :global([data-theme="atelier"]) h1 {
    font-size: clamp(32px, 5vw, 64px);
  }
</style>
