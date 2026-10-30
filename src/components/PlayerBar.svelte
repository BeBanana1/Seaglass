<script lang="ts">
  import { player } from "../lib/stores/player.svelte";
  import { fullTitle } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import HiresOffer from "./HiresOffer.svelte";
  import Icon from "./Icon.svelte";
  import LikeButton from "./LikeButton.svelte";
  import Progress from "./Progress.svelte";
  import SignalPath from "./SignalPath.svelte";
  import Transport from "./Transport.svelte";
  import Volume from "./Volume.svelte";

  let {
    queueOpen,
    onToggleQueue,
    onOpenNowPlaying,
  }: { queueOpen: boolean; onToggleQueue: () => void; onOpenNowPlaying: () => void } = $props();

  const s = $derived(player.state);
</script>

<footer class="panel">
  <HiresOffer />
  <div class="now">
    {#if s.track}
      <button class="cover" onclick={onOpenNowPlaying} title={t.player.openNowPlaying}>
        {#if s.track.album.coverUrl}<img src={s.track.album.coverUrl} alt="" />{/if}
        <span class="expand"><Icon name="add" size={14} /></span>
      </button>
      <div class="meta">
        <span class="title">{fullTitle(s.track)}</span>
        <span class="sub">
          {#each s.track.artists as artist, i (artist.id)}
            {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
          {/each}
        </span>
      </div>
      <LikeButton track={s.track} />
    {:else}
      <span class="sub">{t.player.nothingPlaying}</span>
    {/if}
  </div>

  <div class="center">
    <Transport />
    <Progress />
  </div>

  <div class="right">
    <div class="signal-full"><SignalPath dense /></div>
    <button class="signal-chip" onclick={onOpenNowPlaying} title={t.player.signalPath}>
      <SignalPath compact />
    </button>
    <Volume />
    <button class="toggle" class:on={queueOpen} onclick={onToggleQueue} title={t.player.queue}>
      <Icon name="queue" />
    </button>
  </div>
</footer>

<style>
  footer {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.6fr) minmax(0, 1fr);
    align-items: center;
    gap: 20px;
    height: 100%;
    padding: 0 18px;
  }

  .now {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  .cover {
    position: relative;
    flex-shrink: 0;
    width: 56px;
    height: 56px;
    padding: 0;
    border-radius: 6px;
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow: 0 4px 16px rgb(0 0 0 / 0.3);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
    transition: transform 0.3s;
  }

  .cover:hover img {
    transform: scale(1.06);
  }

  .expand {
    position: absolute;
    top: 4px;
    right: 4px;
    display: grid;
    place-items: center;
    width: 20px;
    height: 20px;
    border-radius: 50%;
    color: var(--text);
    background: rgb(0 0 0 / 0.55);
    opacity: 0;
    transition: opacity var(--motion-fast);
  }

  .cover:hover .expand {
    opacity: 1;
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title,
  .sub {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 600;
  }

  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }

  .link:hover {
    color: var(--text);
    text-decoration: underline;
  }

  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
  }

  .right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    min-width: 0;
  }

  .signal-full {
    display: none;
  }

  .toggle {
    padding: 6px;
    color: var(--text-muted);
  }

  .toggle:hover {
    color: var(--text);
  }

  .toggle.on {
    color: var(--accent);
  }

  /* Studio : la chaîne audio complète remplace la pastille, la pochette est carrée.
     La colonne de droite porte aussi le volume : elle prend un peu sur le centre. */
  :global([data-theme="studio"]) footer {
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) minmax(0, 1.5fr);
    border-top: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .signal-full {
    display: block;
    min-width: 0;
  }

  :global([data-theme="studio"]) .signal-chip {
    display: none;
  }

  :global([data-theme="studio"]) .cover {
    border-radius: 0;
    box-shadow: none;
  }

  :global([data-theme="atelier"]) footer {
    border-top: 1px solid var(--text);
  }

  :global([data-theme="atelier"]) .cover {
    border-radius: 0;
    box-shadow: none;
  }

  :global([data-theme="atelier"]) .title {
    font-family: var(--display-family);
    font-size: 15px;
    font-weight: 500;
  }
</style>
