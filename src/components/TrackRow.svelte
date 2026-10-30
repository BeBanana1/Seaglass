<script lang="ts">
  import { commands, type Track } from "../lib/ipc";
  import { formatTime, fullTitle } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import { closeMenu, menu, menuPosition, openMenu } from "../lib/stores/menu.svelte";
  import { trackMenu } from "../lib/trackMenu";
  import Icon from "./Icon.svelte";

  let {
    track,
    number,
    showCover = true,
    active = false,
    compact = false,
    actions = true,
    onPlay,
    onRemove,
  }: {
    track: Track;
    /** Numéro affiché devant le titre (position dans l'album ou la playlist). */
    number?: number;
    /** Pochette en début de ligne (inutile sur une page d'album). */
    showCover?: boolean;
    active?: boolean;
    /** Variante étroite (panneau de file) : sans colonne album. */
    compact?: boolean;
    /** Boutons « Lire ensuite » / « Ajouter à la file » / « Radio » (ou « Retirer » si `onRemove`) ; le menu « … » reste. */
    actions?: boolean;
    onPlay?: () => void;
    onRemove?: () => void;
  } = $props();

  let rowEl = $state<HTMLElement>();
  let moreButton = $state<HTMLButtonElement>();
  const menuOpen = $derived(!!rowEl && menu.open?.owner === rowEl);

  // Les mêmes entrées au clic droit et sur le bouton « … » (#52).
  function entries() {
    return trackMenu(track, { onPlay, onRemove, queueable: actions });
  }

  function onContextmenu(e: MouseEvent) {
    e.preventDefault();
    const row = e.currentTarget as HTMLElement;
    const focused = row.contains(document.activeElement) ? (document.activeElement as HTMLElement) : null;
    openMenu({ ...menuPosition(e, focused ?? row), entries: entries(), from: focused, owner: row });
  }

  function toggleMore() {
    if (!moreButton) return;
    if (menu.open?.from === moreButton) return closeMenu();
    const rect = moreButton.getBoundingClientRect();
    openMenu({ x: rect.left, y: rect.bottom + 4, entries: entries(), from: moreButton, owner: rowEl });
  }
</script>

<div
  class="row"
  class:active
  class:compact
  class:numbered={number !== undefined}
  class:no-cover={!showCover}
  class:menu-open={menuOpen}
  bind:this={rowEl}
  role="listitem"
  ondblclick={onPlay}
  oncontextmenu={onContextmenu}
>
  {#if number !== undefined}<span class="number">{number}</span>{/if}
  <div class="cover" class:hidden={!showCover}>
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
    <span class="sub"
      >{#each track.artists as artist, i (artist.id)}{#if i > 0},&nbsp;{/if}<button
          class="link"
          onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button
        >{/each}</span
    >
  </div>
  {#if !compact}
    <span class="album">
      <button class="link" onclick={() => go({ name: "album", id: track.album.id })}>{track.album.title}</button>
    </span>
  {/if}
  <div class="actions">
    {#if !actions}
      <!-- seulement le menu -->
    {:else if onRemove}
      <button onclick={onRemove} title="Retirer de la file"><Icon name="close" size={16} /></button>
    {:else}
      <button onclick={() => commands.playNext(track.id)} title="Lire ensuite">
        <Icon name="playNext" size={16} />
      </button>
      <button onclick={() => commands.addToQueue(track.id)} title="Ajouter à la file">
        <Icon name="add" size={16} />
      </button>
      <button onclick={() => go({ name: "radio", seed: "track", id: track.id })} title="Radio du titre">
        <Icon name="radio" size={16} />
      </button>
    {/if}
    <button
      class="more"
      bind:this={moreButton}
      onclick={toggleMore}
      title="Plus d'actions"
      aria-label="Plus d'actions"
      aria-haspopup="menu"
      aria-expanded={menuOpen && menu.open?.from === moreButton}
    >
      <Icon name="more" size={16} />
    </button>
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

  .row.no-cover .cover.hidden {
    width: 32px;
    background: none;
  }

  .row.no-cover .cover.hidden img {
    display: none;
  }

  .row.no-cover .cover-play {
    background: none;
    color: var(--accent);
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

  .row:hover,
  .row.menu-open {
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
  .row.menu-open .cover-play,
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
  .actions:focus-within,
  .row.menu-open .actions {
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
