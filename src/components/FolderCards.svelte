<script lang="ts">
  // Cartes des dossiers de playlists (#331), en tête de Bibliothèque › Playlists. Rien n'est
  // rendu sans dossier. Même grille et mêmes classes que les cartes (`.card`) : les thèmes
  // les habillent pareil ; `.folder-card` en plus.
  import type { PlaylistFolder, PlaylistSummary } from "../lib/ipc";
  import { folderPlaylists } from "../lib/stores/folders.svelte";
  import { go } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let { items, playlists }: { items: PlaylistFolder[]; playlists: PlaylistSummary[] } = $props();
</script>

{#if items.length > 0}
  <div class="items">
    {#each items as folder (folder.id)}
      {@const inside = folderPlaylists(folder, playlists)}
      <button class="card folder-card" onclick={() => go({ name: "folder", id: folder.id })}>
        <div class="image">
          <!-- Jusqu'à quatre pochettes du dossier en mosaïque, sinon l'icône. -->
          {#if inside.some((p) => p.imageUrl)}
            <div class="mosaic" class:single={inside.filter((p) => p.imageUrl).length < 4}>
              {#each inside.filter((p) => p.imageUrl).slice(0, 4) as p (p.uuid)}
                <img src={p.imageUrl} alt="" loading="lazy" />
              {/each}
            </div>
          {/if}
          <span class="badge-icon">
            <Icon name="folder" size={inside.some((p) => p.imageUrl) ? 20 : 48} />
          </span>
        </div>
        <span class="title">{folder.name}</span>
        <span class="sub">{t.library.folders.count(inside.length)}</span>
      </button>
    {/each}
  </div>
{/if}

<style>
  .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
    margin-bottom: 8px;
  }

  :global(:root[data-platform="android"]) .items {
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 4px;
    width: 100%;
    padding: 10px;
    border-radius: var(--radius);
    text-align: left;
  }

  .card:hover {
    background: var(--bg-hover);
  }

  .image {
    position: relative;
    display: grid;
    place-items: center;
    aspect-ratio: 1;
    margin-bottom: 6px;
    border-radius: var(--radius-card);
    overflow: hidden;
    color: var(--text-muted);
    background: var(--bg-raised);
    box-shadow: var(--shadow-cover, none);
  }

  .mosaic {
    position: absolute;
    inset: 0;
    display: grid;
    grid-template-columns: 1fr 1fr;
  }

  .mosaic.single {
    grid-template-columns: 1fr;
  }

  .mosaic.single img:not(:first-child) {
    display: none;
  }

  .mosaic img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  /* Avec des pochettes : petite pastille en bas à gauche, pour distinguer un dossier
     d'une playlist. */
  .mosaic + .badge-icon {
    position: absolute;
    left: 8px;
    bottom: 8px;
    display: grid;
    place-items: center;
    padding: 5px;
    border-radius: 6px;
    color: var(--text);
    background: color-mix(in srgb, var(--bg) 80%, transparent);
  }

  .title,
  .sub {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .title {
    font-weight: 500;
  }

  .sub {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
