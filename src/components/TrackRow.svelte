<script lang="ts">
  import { commands, type Track } from "../lib/ipc";
  import { coverSmall, formatTime, fullTitle } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import {
    closeMenu,
    menu,
    menuPosition,
    openMenu,
    type MenuEntry,
  } from "../lib/stores/menu.svelte";
  import { trackMenu } from "../lib/trackMenu";
  import Icon from "./Icon.svelte";
  import LikeButton from "./LikeButton.svelte";
  import { isLaterPending, isSavedForLater, toggleLater } from "../lib/stores/later.svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { swipeRight } from "../lib/swipe";
  import { player } from "../lib/stores/player.svelte";
  import { eggs } from "../lib/eggs/eggs.svelte";
  import { parsecs } from "../lib/eggs/playback";

  let {
    track,
    number,
    showCover = true,
    active = false,
    compact = false,
    actions = true,
    onPlay,
    onRemove,
    onMoveUp,
    onMoveDown,
    onRemoveFromPlaylist,
    highlighted = false,
    addedBy,
    laterButton = false,
    note,
    selected = false,
    selecting = false,
    onSelect,
    selectionMenu,
  }: {
    track: Track;
    /** Numéro affiché devant le titre (position dans l'album ou la playlist). */
    number?: number;
    /** Pochette en début de ligne (inutile sur une page d'album). */
    showCover?: boolean;
    /** Titre en cours (#241) : fond teinté, barre d'accent et égaliseur à la place de la pochette / du numéro. */
    active?: boolean;
    /** Variante étroite (panneau de file) : sans colonne album. */
    compact?: boolean;
    /** Boutons « Lire ensuite » / « Ajouter à la file » / « Radio » (ou « Retirer » si `onRemove`) ; le menu « … » reste. */
    actions?: boolean;
    onPlay?: () => void;
    onRemove?: () => void;
    /** Ligne de la file manuelle : « Monter » / « Descendre » dans le menu (#80). */
    onMoveUp?: () => void;
    onMoveDown?: () => void;
    /** Ligne d'une de vos playlists : « Retirer de cette playlist » dans le menu. */
    onRemoveFromPlaylist?: () => void;
    /** Titre désigné par un lien collé (#203) : marqué et amené au centre de l'écran. */
    highlighted?: boolean;
    /** File manuelle pendant une Jam (#25) : invité qui a ajouté le titre. */
    addedBy?: string;
    /** Liste « Plus tard » (#249) : signet plein à côté du cœur, pour retirer le titre en un clic. */
    laterButton?: boolean;
    /** Courte mention sous les artistes (nombre d'écoutes des statistiques, #326). */
    note?: string;
    /** Titre coché dans la sélection multiple (#332). */
    selected?: boolean;
    /** Sélection en cours dans la liste : sur Android, un appui coche au lieu de lire. */
    selecting?: boolean;
    /** Liste qui permet la sélection : `toggle` (Ctrl+clic, appui), `extend` (Maj+clic). */
    onSelect?: (how: "toggle" | "extend") => void;
    /** Menu d'un titre sélectionné : actions sur toute la sélection. */
    selectionMenu?: () => MenuEntry[];
  } = $props();

  const laterTarget = $derived({ kind: "track" as const, id: track.id });
  const saved = $derived(laterButton && isSavedForLater(laterTarget));

  $effect(() => {
    if (highlighted) rowEl?.scrollIntoView({ block: "center" });
  });

  let rowEl = $state<HTMLElement>();
  let moreButton = $state<HTMLButtonElement>();
  const menuOpen = $derived(!!rowEl && menu.open?.owner === rowEl);

  // Les mêmes entrées au clic droit et sur le bouton « … » (#52).
  function entries() {
    if (selected && selectionMenu) return selectionMenu();
    return trackMenu(track, {
      onPlay,
      onRemove,
      onMoveUp,
      onMoveDown,
      queueable: actions,
      onRemoveFromPlaylist,
      onSelect: onSelect && (() => onSelect("toggle")),
    });
  }

  function onContextmenu(e: MouseEvent) {
    e.preventDefault();
    const row = e.currentTarget as HTMLElement;
    const focused = row.contains(document.activeElement)
      ? (document.activeElement as HTMLElement)
      : null;
    openMenu({ ...menuPosition(e, focused ?? row), entries: entries(), from: focused, owner: row });
  }

  // Android (#24) : un appui sur la ligne lance le titre (double-clic sur le bureau), hors
  // boutons et liens. L'appui long émet `contextmenu` et ouvre le menu.
  // Sélection multiple (#332) : Ctrl+clic et Maj+clic sur le bureau ; sur Android, pendant
  // une sélection, l'appui coche ou décoche au lieu de lire.
  function onTap(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("button, a, input")) return;
    if (onSelect && !platform.mobile && (e.ctrlKey || e.metaKey || e.shiftKey)) {
      onSelect(e.shiftKey ? "extend" : "toggle");
      return;
    }
    if (!platform.mobile) return;
    if (selecting && onSelect) onSelect("toggle");
    else onPlay?.();
  }

  // Maj+clic ne doit pas sélectionner le texte entre deux lignes.
  function onMousedown(e: MouseEvent) {
    if (onSelect && e.shiftKey && !platform.mobile) e.preventDefault();
  }

  // Android (#174) : glisser vers la droite ajoute le titre à la file. Pas sur les lignes de la
  // file elle-même ni de « À suivre » (sans actions de file), ni pendant une sélection.
  const swipeable = $derived(platform.mobile && actions && !onRemove && !selecting);

  async function swipeToQueue() {
    try {
      await commands.addToQueue(track.id);
      pushToast({ level: "info", message: t.views.row.queued(fullTitle(track)) });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function toggleMore() {
    if (!moreButton) return;
    if (menu.open?.from === moreButton) return closeMenu();
    const rect = moreButton.getBoundingClientRect();
    openMenu({
      x: rect.left,
      y: rect.bottom + 4,
      entries: entries(),
      from: moreButton,
      owner: rowEl,
    });
  }
</script>

<!-- L'appui sur la ligne (Android) double le bouton de lecture de la pochette, accessible au clavier. -->
<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_noninteractive_element_interactions -->
<div
  class="row"
  class:active
  class:highlighted
  class:compact
  class:numbered={number !== undefined}
  class:no-cover={!showCover}
  class:menu-open={menuOpen}
  class:swipeable
  class:selected
  bind:this={rowEl}
  use:swipeRight={{ enabled: swipeable, onSwipe: swipeToQueue }}
  role="listitem"
  ondblclick={platform.mobile || selecting ? undefined : onPlay}
  onclick={onTap}
  onmousedown={onMousedown}
  oncontextmenu={onContextmenu}
>
  {#snippet nowBars()}
    <!-- Lu seulement par la ligne en cours : les autres ne suivent pas `player.playing`. -->
    <span class="now-bars" class:paused={!player.playing} aria-hidden="true">
      <i></i><i></i><i></i>
    </span>
  {/snippet}
  {#if number !== undefined}
    <span class="number">
      {#if active}{@render nowBars()}{:else if number === 13 && eggs.date === "friday13"}12<sup
          class="bis"
        >
          {t.eggs.thirteen}
        </sup>{:else}{number}{/if}
    </span>
  {/if}
  <div class="cover" class:hidden={!showCover}>
    {#if track.album.coverUrl}
      <img src={coverSmall(track.album)} alt="" loading="lazy" />
    {/if}
    {#if active && number === undefined}{@render nowBars()}{/if}
    {#if selected}<span class="selected-mark" aria-hidden="true">
        <Icon name="check" size={18} />
      </span>{/if}
    {#if onPlay}
      <button class="cover-play" onclick={onPlay} aria-label={t.views.row.play(track.title)}>
        <Icon name="play" size={16} />
      </button>
    {/if}
  </div>
  <div class="main">
    <span class="title">
      {fullTitle(track)}
      {#if track.explicit}<span class="badge muted">E</span>{/if}
      {#if track.ai}<span class="badge muted">{t.common.ai}</span>{/if}
      {#if track.hires}<span class="badge">HI-RES</span>{/if}
    </span>
    <!-- Android (#180) : les artistes ne sont pas des liens, l'appui tombe sur la ligne et lance
         le titre ; ils restent dans le menu (appui long ou « … ») et dans « En lecture ». -->
    <span class="sub">
      {#if platform.mobile}{track.artists
          .map((a) => a.name)
          .join(
            ", ",
          )}{:else}{#each track.artists as artist, i (artist.id)}{#if i > 0},&nbsp;{/if}<button
            class="link"
            onclick={() => go({ name: "artist", id: artist.id })}
          >
            {artist.name}
          </button>{/each}{/if}
    </span>
    {#if addedBy}<span class="added-by">{t.jam.addedBy(addedBy)}</span>{/if}
    {#if note}<span class="note">{note}</span>{/if}
  </div>
  {#if !compact}
    <span class="album">
      <button class="link" onclick={() => go({ name: "album", id: track.album.id })}>
        {track.album.title}
      </button>
    </span>
  {/if}
  <div class="actions">
    {#if !actions}
      <!-- seulement le menu -->
    {:else if onRemove}
      <button onclick={onRemove} title={t.menu.removeFromQueue}>
        <Icon name="close" size={16} />
      </button>
    {:else}
      <button
        class="queue-action"
        onclick={() => commands.playNext(track.id)}
        title={t.menu.playNext}
      >
        <Icon name="playNext" size={16} />
      </button>
      <button
        class="queue-action"
        onclick={() => commands.addToQueue(track.id)}
        title={t.menu.addToQueue}
      >
        <Icon name="add" size={16} />
      </button>
      <button
        class="queue-action"
        onclick={() => go({ name: "radio", seed: "track", id: track.id })}
        title={t.menu.trackRadio}
      >
        <Icon name="radio" size={16} />
      </button>
    {/if}
    <button
      class="more"
      bind:this={moreButton}
      onclick={toggleMore}
      title={t.views.row.more}
      aria-label={t.views.row.more}
      aria-haspopup="menu"
      aria-expanded={menuOpen && menu.open?.from === moreButton}
    >
      <Icon name="more" size={16} />
    </button>
    {#if laterButton}
      <!-- Classes du cœur (`like-button liked`) : même place, toujours visible et même
           habillage dans chaque thème que le cœur plein des titres likés. -->
      <button
        class="like-button later-row"
        class:liked={saved}
        class:pending={isLaterPending(laterTarget)}
        onclick={(e) => {
          e.stopPropagation();
          toggleLater(laterTarget);
        }}
        ondblclick={(e) => e.stopPropagation()}
        aria-pressed={saved}
        title={saved ? t.menu.removeFromLater : t.menu.saveForLater}
        aria-label={saved ? t.menu.removeFromLater : t.menu.saveForLater}
      >
        <Icon name={saved ? "bookmarkFilled" : "bookmark"} size={16} />
      </button>
    {/if}
    <LikeButton {track} />
  </div>
  <!-- #399 : recherche « kessel », les durées passent un instant en parsecs. -->
  <span class="duration">
    {eggs.search === "kessel"
      ? t.eggs.parsecs(parsecs(track.duration))
      : formatTime(track.duration)}
  </span>
</div>

<style>
  .row {
    display: grid;
    grid-template-columns: 40px minmax(0, 2fr) minmax(0, 1fr) auto 48px;
    align-items: center;
    gap: 12px;
    padding: 6px 10px;
    border-radius: var(--radius);
    /* Lignes hors écran ni mises en page ni peintes (listes de milliers de titres, #88). La
       hauteur réservée est celle d'une ligne (pochette 40 px + marges), `auto` garde ensuite la
       hauteur mesurée. Contient la peinture : rien d'une ligne ne doit déborder de son cadre. */
    content-visibility: auto;
    contain-intrinsic-size: auto 52px;
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

  /* Vendredi 13 (#399) : la piste 13 devient « 12 bis », comme les étages d'hôtel. */
  .bis {
    font-size: 0.65em;
    white-space: nowrap;
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

  .row.highlighted {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  /* Titre en cours (#241) : la seule couleur du titre se perdait dans certains thèmes. */
  .row.active {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    box-shadow: inset 3px 0 0 var(--accent);
  }

  .row.active:hover,
  .row.active.menu-open {
    background: color-mix(in srgb, var(--accent) 18%, transparent);
  }

  .row.active .title {
    color: var(--accent);
  }

  /* Sélection multiple (#332) : fond plus marqué que la ligne en cours, liseré complet et
     coche sur la pochette. */
  .row.selected,
  .row.selected:hover {
    background: color-mix(in srgb, var(--accent) 24%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 60%, transparent);
  }

  .selected-mark {
    position: absolute;
    inset: 0;
    z-index: 1;
    display: grid;
    place-items: center;
    color: #fff;
    background: color-mix(in srgb, var(--accent) 70%, rgb(0 0 0 / 0.4));
    pointer-events: none;
  }

  .cover.hidden .selected-mark {
    color: var(--accent);
    background: none;
  }

  .now-bars {
    display: inline-flex;
    align-items: flex-end;
    justify-content: center;
    gap: 2px;
    width: 14px;
    height: 12px;
    color: var(--accent);
  }

  .now-bars i {
    width: 3px;
    height: 100%;
    border-radius: 1px;
    background: currentColor;
    transform-origin: bottom;
    animation: now-bars 0.9s ease-in-out infinite alternate;
  }

  .now-bars i:nth-child(2) {
    animation-delay: -0.3s;
  }

  .now-bars i:nth-child(3) {
    animation-delay: -0.6s;
  }

  .now-bars.paused i {
    animation-play-state: paused;
  }

  @keyframes now-bars {
    from {
      transform: scaleY(0.25);
    }
    to {
      transform: scaleY(1);
    }
  }

  /* Thème Désert (#399) : au début d'un album, les barres battent sans rythme régulier. */
  :global(:root[data-egg-walk]) .now-bars i {
    animation-timing-function: steps(3, jump-none);
    animation-duration: 0.53s;
  }

  :global(:root[data-egg-walk]) .now-bars i:nth-child(2) {
    animation-duration: 1.27s;
  }

  :global(:root[data-egg-walk]) .now-bars i:nth-child(3) {
    animation-duration: 0.81s;
  }

  /* Recherche « silence » (#399) : les barres se figent un instant. */
  :global(:root[data-egg="silence"]) .now-bars i {
    animation-play-state: paused;
  }

  :global(:root[data-motion="reduced"]) .now-bars i {
    transform: scaleY(0.7);
  }

  /* Sur la pochette : voile sombre et barres claires, centrés. */
  .cover .now-bars {
    position: absolute;
    inset: 0;
    width: auto;
    height: auto;
    padding: 13px 12px;
    color: #fff;
    background: rgb(0 0 0 / 0.5);
  }

  .cover.hidden .now-bars {
    padding: 14px 9px;
    color: var(--accent);
    background: none;
  }

  /* Le bouton de lecture prend la place au survol (bureau : sur Android, `:hover` reste
     collé après un appui et le bouton est masqué). */
  :global(:root:not([data-platform="android"])) .row:hover .cover .now-bars,
  .row.menu-open .cover .now-bars {
    visibility: hidden;
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
  .note {
    font-size: 12px;
    color: var(--text-muted);
  }

  .added-by {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
    font-size: 12px;
    color: var(--accent);
  }

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
  }

  /* Boutons visibles au survol ; un cœur plein reste affiché. */
  .actions > :global(*) {
    opacity: 0;
  }

  .row:hover .actions > :global(*),
  .actions:focus-within > :global(*),
  .row.menu-open .actions > :global(*),
  .actions > :global(.like-button.liked) {
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

  .actions .later-row.liked {
    color: var(--accent);
  }

  .actions .later-row.pending {
    opacity: 0.5;
    cursor: progress;
  }

  .duration {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  /* Android (#24) : pas de survol. « … » et le cœur restent affichés ; les boutons de file
     (aussi dans le menu), l'album et la durée laissent la place au titre. */
  :global(:root[data-platform="android"]) .row,
  :global(:root[data-platform="android"]) .row.compact {
    grid-template-columns: 40px minmax(0, 1fr) auto;
    gap: 10px;
    padding: 6px 4px;
  }

  :global(:root[data-platform="android"]) .row.numbered,
  :global(:root[data-platform="android"]) .row.compact.numbered {
    grid-template-columns: 22px 40px minmax(0, 1fr) auto;
  }

  /* Page d'album : pas de colonne de pochette vide. */
  :global(:root[data-platform="android"]) .row.no-cover {
    grid-template-columns: minmax(0, 1fr) auto;
  }

  :global(:root[data-platform="android"]) .row.no-cover.numbered {
    grid-template-columns: 22px minmax(0, 1fr) auto;
  }

  :global(:root[data-platform="android"]) .row.no-cover .cover {
    display: none;
  }

  /* Au doigt, la ligne touchée garde `:hover` jusqu'au prochain appui ailleurs : pas de fond
     de survol. La ligne en cours garde le sien (#291 : lancer un titre effaçait son fond
     jusqu'à l'appui suivant, sur Pause par exemple). */
  :global(:root[data-platform="android"]) .row:hover:not(.menu-open, .active, .selected) {
    background: none;
  }

  :global(:root[data-platform="android"]) .row.active:hover:not(.menu-open, .selected) {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  :global(:root[data-platform="android"]) .album,
  :global(:root[data-platform="android"]) .duration,
  :global(:root[data-platform="android"]) .queue-action,
  :global(:root[data-platform="android"]) .cover-play {
    display: none;
  }

  :global(:root[data-platform="android"]) .actions > :global(*) {
    opacity: 1;
  }

  :global(:root[data-platform="android"]) .actions button {
    padding: 8px;
  }

  /* Glisser vers la droite = ajouter à la file (#174). Le défilement vertical reste au
     navigateur ; le contenu de la ligne suit le doigt (`--swipe-x`, posé par `swipeRight`) et
     découvre un « + », à l'accent une fois le seuil franchi. Rien n'est transformé hors du
     geste : pas de contexte d'empilement ajouté aux lignes au repos. */
  .row.swipeable {
    position: relative;
    touch-action: pan-y;
  }

  .row:is(:global(.swiping), :global(.swipe-release)) > * {
    transform: translateX(var(--swipe-x, 0px));
  }

  .row:global(.swipe-release) > * {
    transition: transform 180ms ease;
  }

  .row:is(:global(.swiping), :global(.swipe-release))::before {
    content: "+";
    position: absolute;
    top: 50%;
    left: 14px;
    translate: 0 -50%;
    font-size: 28px;
    font-weight: 300;
    line-height: 1;
    color: var(--text-muted);
    opacity: var(--swipe-fade, 0);
  }

  .row:global(.swipe-armed)::before {
    color: var(--accent);
    font-weight: 600;
  }

  .row:global(.swipe-armed) {
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }
</style>
