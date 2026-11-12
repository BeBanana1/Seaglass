<script lang="ts">
  import type { MediaItem } from "../lib/ipc";
  import Card from "./Card.svelte";

  let {
    items,
    title,
    row = false,
    notes,
  }: {
    items: MediaItem[];
    title?: string;
    /** Une seule ligne défilante (accueil, recherche) plutôt qu'une grille. */
    row?: boolean;
    /** Mention sous chaque carte (nombre d'écoutes des statistiques, #326), dans l'ordre de `items`. */
    notes?: string[];
  } = $props();
</script>

{#if items.length > 0}
  <section>
    {#if title}<h2>{title}</h2>{/if}
    <div class="items" class:row>
      {#each items as item, i (`${item.kind}-${i}`)}
        {#if notes}
          <div class="noted">
            <Card {item} />
            <span class="card-note">{notes[i]}</span>
          </div>
        {:else}
          <Card {item} />
        {/if}
      {/each}
    </div>
  </section>
{/if}

<style>
  section {
    margin-bottom: 28px;
  }

  h2 {
    margin: 0 10px 8px;
    font-size: 17px;
    font-weight: 600;
  }

  .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  }

  .noted {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .card-note {
    margin: -6px 12px 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .items.row {
    grid-auto-flow: column;
    grid-auto-columns: 170px;
    grid-template-columns: none;
    overflow-x: auto;
    scrollbar-width: thin;
  }

  /* Android (#24) : deux cartes par ligne sur un téléphone (170 px n'en laissait qu'une). */
  :global(:root[data-platform="android"]) .items {
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  }

  :global(:root[data-platform="android"]) .items.row {
    grid-auto-columns: 148px;
    grid-template-columns: none;
  }
</style>
