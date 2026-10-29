<script lang="ts">
  // Vue immersive « En lecture » : grande pochette, chaîne audio, à suivre.
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { fullTitle } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import Backdrop from "../components/backdrops/Backdrop.svelte";
  import Icon from "../components/Icon.svelte";
  import Progress from "../components/Progress.svelte";
  import SignalPath from "../components/SignalPath.svelte";
  import Transport from "../components/Transport.svelte";

  let { onClose }: { onClose: () => void } = $props();

  const s = $derived(player.state);
  const upNext = $derived([...queue.view.manual, ...queue.view.upcoming].slice(0, 5));

  function open(route: Parameters<typeof go>[0]) {
    go(route);
    onClose();
  }
</script>

<div class="now-playing" role="dialog" aria-modal="true" aria-label="En lecture">
  <Backdrop context="nowplaying" />
  <button class="close" onclick={onClose} title="Fermer (Échap)">
    <Icon name="close" size={20} />
  </button>

  {#if s.track}
    {@const track = s.track}
    <div class="layout">
      <div class="art">
        {#if track.album.coverUrl}
          <img src={track.album.coverUrl.replace("320x320", "640x640")} alt="" />
        {/if}
      </div>
      <div class="info">
        <span class="label">En lecture</span>
        <h1 class="display">{fullTitle(track)}</h1>
        <div class="who">
          {#each track.artists as artist, i (artist.id)}
            {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => open({ name: "artist", id: artist.id })}>{artist.name}</button>
          {/each}
        </div>
        <button class="link album" onclick={() => open({ name: "album", id: track.album.id })}>{track.album.title}</button>

        <div class="controls">
          <Progress large />
          <Transport large />
        </div>

        <div class="signal"><SignalPath /></div>

        {#if upNext.length > 0}
          <div class="next">
            <span class="label">À suivre</span>
            <ol>
              {#each upNext as t, i (`${i}-${t.id}`)}
                <li>
                  {#if t.album.coverUrl}<img src={t.album.coverUrl} alt="" />{/if}
                  <span class="t">{fullTitle(t)}</span>
                  <span class="a">{t.artists.map((a) => a.name).join(", ")}</span>
                </li>
              {/each}
            </ol>
          </div>
        {/if}
      </div>
    </div>
  {:else}
    <p class="empty">Rien en lecture.</p>
  {/if}
</div>

<style>
  .now-playing {
    position: fixed;
    inset: 0;
    z-index: 50;
    display: grid;
    place-items: center;
    overflow: auto;
    background: var(--bg);
    animation: rise 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(24px) scale(0.98);
    }
  }

  .close {
    position: absolute;
    top: 18px;
    right: 18px;
    z-index: 2;
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    color: var(--text);
    background: var(--bg-raised);
  }

  .close:hover {
    background: var(--bg-hover);
  }

  .layout {
    position: relative;
    display: grid;
    grid-template-columns: minmax(240px, 46vh) minmax(320px, 560px);
    align-items: center;
    gap: clamp(32px, 6vw, 88px);
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
    color: var(--text-muted);
  }

  @media (max-width: 900px) {
    .layout {
      grid-template-columns: minmax(0, 420px);
      padding: 72px 24px 24px;
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

  :global([data-theme="studio"]) .close {
    border-radius: 0;
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
