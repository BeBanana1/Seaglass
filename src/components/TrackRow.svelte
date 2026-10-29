<script lang="ts">
  import { commands, type Track } from "../lib/ipc";
  import { formatTime, fullTitle } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import Icon from "./Icon.svelte";

  let {
    track,
    number,
    active = false,
    compact = false,
    actions = true,
    onPlay,
    onRemove,
  }: {
    track: Track;
    /** Numéro affiché devant le titre (position dans l'album ou la playlist). */
    number?: number;
    active?: boolean;
    /** Variante étroite (panneau de file) : sans colonne album. */
    compact?: boolean;
    /** Boutons « Lire ensuite » / « Ajouter à la file » (ou « Retirer » si `onRemove`). */
    actions?: boolean;
    onPlay?: () => void;
    onRemove?: () => void;
  } = $props();
</script>

<div
  class="row"
  class:active
  class:compact
  class:numbered={number !== undefined}
  role="listitem"
  ondblclick={onPlay}
>
  {#if number !== undefined}<span class="number">{number}</span>{/if}
  <div class="cover">
    {#if track.album.coverUrl}
      <img src={track.album.coverUrl} alt="" loading="lazy" />
    {/if}
    {#if onPlay}
      <button class="cover-play" onclick={onPlay} aria-label="Lire {track.title}">
        <Icon name="play" size={16} />
      </button>
    {/if}
  </div>
  <div class="main">
    <span class="title">
      {fullTitle(track)}
      {#if track.explicit}<span class="badge muted">E</span>{/if}
      {#if track.hires}<span class="badge">HI-RES</span>{/if}
    </span>
    <span class="sub">
      {#each track.artists as artist, i (artist.id)}
        {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
      {/each}
    </span>
  </div>
  {#if !compact}
    <span class="album">
      <button class="link" onclick={() => go({ name: "album", id: track.album.id })}>{track.album.title}</button>
    </span>
  {/if}
  <div class="actions">
    {#if !actions}
      <!-- aucune action -->
    {:else if onRemove}
      <button onclick={onRemove} title="Retirer de la file"><Icon name="close" size={16} /></button>
    {:else}
      <button onclick={() => commands.playNext(track.id)} title="Lire ensuite">
        <Icon name="playNext" size={16} />
      </button>
      <button onclick={() => commands.addToQueue(track.id)} title="Ajouter à la file">
        <Icon name="add" size={16} />
      </button>
    {/if}
  </div>
  <span class="duration">{formatTime(track.duration)}</span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 2fr) minmax(0, 1fr) auto 48px;
    align-items: center;
    gap: 12px;
    padding: 6px 10px;
    border-radius: var(--radius);
  }

  .row.numbered {
    grid-template-columns: 24px 40px minmax(0, 2fr) minmax(0, 1fr) auto 48px;
  }

  .row.compact.numbered {
    grid-template-columns: 24px 40px minmax(0, 1fr) auto 48px;
  }

  .number {
    text-align: right;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
  }

  .link:hover {
    color: var(--text);
    text-decoration: underline;
  }

  .row.compact {
    grid-template-columns: 40px minmax(0, 1fr) auto 40px;
  }

  .row:hover {
    background: var(--bg-hover);
  }

  .row.active .title {
    color: var(--accent);
  }

  .cover {
    position: relative;
    width: 40px;
    height: 40px;
    border-radius: 4px;
    overflow: hidden;
    background: var(--bg-raised);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .cover-play {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--text);
    background: rgb(0 0 0 / 0.55);
    opacity: 0;
  }

  .row:hover .cover-play,
  .cover-play:focus-visible {
    opacity: 1;
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

  .actions {
    display: flex;
    gap: 4px;
    opacity: 0;
  }

  .row:hover .actions,
  .actions:focus-within {
    opacity: 1;
  }

  .actions button {
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .actions button:hover {
    color: var(--text);
    background: var(--border);
  }

  .duration {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }
</style>
