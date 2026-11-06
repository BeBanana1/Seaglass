<script lang="ts">
  import type { Snippet } from "svelte";
  import { commands, type LikeTarget, type Pin, type PlaySource, type TrackId } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";
  import PinButton from "./PinButton.svelte";
  import FavoriteButton from "./FavoriteButton.svelte";
  import LaterButton from "./LaterButton.svelte";
  import { openPlaylistPicker } from "../lib/stores/playlistPicker.svelte";

  let {
    image,
    eyebrow,
    title,
    round = false,
    trackIds = [],
    source,
    queueable = false,
    pin,
    favorite,
    later,
    meta,
    description,
    onRadio,
    onEdit,
  }: {
    image: string | null;
    /** Type de page (« Album », « Playlist »…). */
    eyebrow: string;
    title: string;
    round?: boolean;
    /** Contexte lancé par les boutons Lecture / Aléatoire. */
    trackIds?: TrackId[];
    /** Origine de ce contexte, affichée dans « Joué depuis » (#124). */
    source?: PlaySource;
    /** Affiche « Lire ensuite » / « Ajouter à la file » pour tous les titres (album, playlist). */
    queueable?: boolean;
    /** Bouton Épingler dans la barre latérale (playlist). */
    pin?: Pin;
    /** Ajout aux favoris (album, playlist d'un autre) ou « Suivre » (artiste), #81. */
    favorite?: LikeTarget;
    /** « Plus tard » (album, #227). */
    later?: LikeTarget;
    meta?: Snippet;
    description?: string | null;
    /** Bouton « Radio » (page d'artiste). */
    onRadio?: () => void;
    /** Bouton « Modifier » (votre playlist, #201). */
    onEdit?: () => void;
  } = $props();

  async function play(shuffle: boolean) {
    try {
      await commands.playTracks(trackIds, 0, shuffle, source ?? null);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function enqueue(next: boolean) {
    try {
      await commands.enqueueTracks(trackIds, next);
      pushToast({ level: "info", message: t.views.header.enqueued(trackIds.length, next) });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<header>
  <div class="image" class:round>
    {#if image}<img src={image} alt="" />{/if}
  </div>
  <div class="info">
    <span class="eyebrow">{eyebrow}</span>
    <h1>{title}</h1>
    {#if meta}<div class="meta">{@render meta()}</div>{/if}
    {#if description}<p class="description">{description}</p>{/if}
    {#if trackIds.length > 0 || favorite || later || onEdit}
      <div class="actions">
        {#if trackIds.length > 0}
          <button class="primary" onclick={() => play(false)}>
            <Icon name="play" size={16} /> {t.common.play}
          </button>
          <button class="secondary" onclick={() => play(true)}>
            <Icon name="shuffle" size={16} /> {t.common.shuffle}
          </button>
        {/if}
        {#if favorite}<FavoriteButton target={favorite} />{/if}
        {#if later}<LaterButton target={later} />{/if}
        {#if queueable && trackIds.length > 0}
          <button class="secondary icon" onclick={() => enqueue(true)} title={t.views.header.playNext} aria-label={t.views.header.playNext}>
            <Icon name="playNext" size={18} />
          </button>
          <button
            class="secondary icon"
            onclick={() => enqueue(false)}
            title={t.views.header.addToQueue}
            aria-label={t.views.header.addToQueue}
          >
            <Icon name="add" size={18} />
          </button>
          <button
            class="secondary icon"
            onclick={() => openPlaylistPicker({ trackIds, label: `${title} · ${t.common.tracks(trackIds.length)}` })}
            title={t.views.header.addToPlaylist}
            aria-label={t.views.header.addToPlaylist}
          >
            <Icon name="playlistAdd" size={18} />
          </button>
        {/if}
        {#if pin}<PinButton {pin} />{/if}
        {#if onEdit}
          <button class="secondary icon edit-playlist" onclick={onEdit} title={t.views.header.edit} aria-label={t.views.header.edit}>
            <Icon name="edit" size={18} />
          </button>
        {/if}
        {#if onRadio}
          <button class="secondary" onclick={onRadio}>
            <Icon name="radio" size={16} /> {t.views.header.radio}
          </button>
        {/if}
      </div>
    {/if}
  </div>
</header>

<style>
  header {
    display: flex;
    align-items: flex-end;
    gap: 24px;
    margin-bottom: 24px;
  }

  .image {
    flex-shrink: 0;
    width: 200px;
    height: 200px;
    border-radius: var(--radius-cover, 8px);
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow: var(--shadow-cover, 0 8px 24px rgb(0 0 0 / 0.4));
  }

  .image.round {
    border-radius: 50%;
  }

  img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  .eyebrow {
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  h1 {
    margin: 0;
    font-size: 30px;
    font-weight: 700;
    line-height: 1.15;
  }

  .meta {
    color: var(--text-muted);
  }

  .description {
    margin: 0;
    max-width: 720px;
    font-size: 13px;
    color: var(--text-muted);
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    overflow: hidden;
  }

  .actions {
    display: flex;
    gap: 10px;
    margin-top: 8px;
  }

  .actions button,
  .actions :global(.pin-button),
  .actions :global(.favorite-button),
  .actions :global(.later-button) {
    clip-path: var(--control-clip);
  }

  .actions button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    border-radius: var(--radius-pill);
    font-weight: 600;
  }

  .primary {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:hover {
    background: var(--accent-strong);
  }

  .secondary {
    border: 1px solid var(--border) !important;
  }

  .secondary:hover {
    background: var(--bg-hover);
  }

  .actions button.icon {
    justify-content: center;
    width: 37px;
    padding: 0;
  }

  :global([data-theme="studio"]) .image,
  :global([data-theme="atelier"]) .image {
    box-shadow: none;
  }

  /* Studio et Atelier : portrait d'artiste carré, comme les pochettes. */
  :global([data-theme="studio"]) .image.round,
  :global([data-theme="atelier"]) .image.round {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) h1 {
    font-size: 40px;
  }

  /* Android (#24, #182) : en-tête centré, image au-dessus du titre. Boutons sur deux lignes :
     Lecture et Aléatoire sur toute la largeur, les autres en dessous, centrés, en cibles de 44 px. */
  :global(:root[data-platform="android"]) header {
    flex-direction: column;
    align-items: center;
    gap: 16px;
    text-align: center;
  }

  :global(:root[data-platform="android"]) .image {
    width: min(220px, 60vw);
    height: min(220px, 60vw);
  }

  :global(:root[data-platform="android"]) .info {
    align-items: center;
    width: 100%;
  }

  :global(:root[data-platform="android"]) h1 {
    font-size: 24px;
  }

  :global(:root[data-platform="android"]) .actions {
    flex-wrap: wrap;
    justify-content: center;
    gap: 12px 16px;
    width: 100%;
  }

  :global(:root[data-platform="android"]) .actions button,
  :global(:root[data-platform="android"]) .actions :global(.pin-button),
  :global(:root[data-platform="android"]) .actions :global(.favorite-button),
  :global(:root[data-platform="android"]) .actions :global(.later-button) {
    justify-content: center;
    min-height: 44px;
  }

  :global(:root[data-platform="android"]) .actions .primary,
  :global(:root[data-platform="android"]) .actions .primary + .secondary {
    /* Deux moitiés de la ligne (écart de 16 px) : les autres boutons passent à la suivante. */
    flex: 1 1 calc(50% - 8px);
    font-size: 15px;
  }

  :global(:root[data-platform="android"]) .actions button.icon,
  :global(:root[data-platform="android"]) .actions :global(.pin-button),
  :global(:root[data-platform="android"]) .actions :global(.favorite-button:not(.follow)),
  :global(:root[data-platform="android"]) .actions :global(.later-button) {
    width: 44px;
    height: 44px;
  }
</style>
