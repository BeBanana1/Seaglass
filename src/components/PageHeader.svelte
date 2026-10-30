<script lang="ts">
  import type { Snippet } from "svelte";
  import { commands, type Pin, type TrackId } from "../lib/ipc";
  import { plural } from "../lib/format";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";
  import PinButton from "./PinButton.svelte";

  let {
    image,
    eyebrow,
    title,
    round = false,
    trackIds = [],
    queueable = false,
    pin,
    meta,
    description,
    onRadio,
  }: {
    image: string | null;
    /** Type de page (« Album », « Playlist »…). */
    eyebrow: string;
    title: string;
    round?: boolean;
    /** Contexte lancé par les boutons Lecture / Aléatoire. */
    trackIds?: TrackId[];
    /** Affiche « Lire ensuite » / « Ajouter à la file » pour tous les titres (album, playlist). */
    queueable?: boolean;
    /** Bouton Épingler dans la barre latérale (playlist). */
    pin?: Pin;
    meta?: Snippet;
    description?: string | null;
    /** Bouton « Radio » (page d'artiste). */
    onRadio?: () => void;
  } = $props();

  async function play(shuffle: boolean) {
    try {
      await commands.playTracks(trackIds, 0, shuffle);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function enqueue(next: boolean) {
    try {
      await commands.enqueueTracks(trackIds, next);
      const added = plural(trackIds.length, "titre");
      const verb = trackIds.length > 1 ? "ajoutés" : "ajouté";
      const where = next ? "en tête de la file" : "à la file";
      pushToast({ level: "info", message: `${added} ${verb} ${where}` });
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
    {#if trackIds.length > 0}
      <div class="actions">
        <button class="primary" onclick={() => play(false)}>
          <Icon name="play" size={16} /> Lecture
        </button>
        <button class="secondary" onclick={() => play(true)}>
          <Icon name="shuffle" size={16} /> Aléatoire
        </button>
        {#if queueable}
          <button class="secondary icon" onclick={() => enqueue(true)} title="Lire ensuite" aria-label="Lire ensuite">
            <Icon name="playNext" size={18} />
          </button>
          <button
            class="secondary icon"
            onclick={() => enqueue(false)}
            title="Ajouter à la file"
            aria-label="Ajouter à la file"
          >
            <Icon name="add" size={18} />
          </button>
        {/if}
        {#if pin}<PinButton {pin} />{/if}
        {#if onRadio}
          <button class="secondary" onclick={onRadio}>
            <Icon name="radio" size={16} /> Radio
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
    border-radius: 8px;
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.4);
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

  .actions button {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 18px;
    border-radius: 999px;
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

  :global([data-theme="studio"]) .actions button,
  :global([data-theme="studio"]) .image {
    border-radius: 0;
  }

  :global([data-theme="studio"]) .image,
  :global([data-theme="atelier"]) .image {
    box-shadow: none;
  }

  :global([data-theme="atelier"]) .actions button {
    border-radius: 2px;
  }

  :global([data-theme="atelier"]) .image {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) h1 {
    font-size: 40px;
  }
</style>
