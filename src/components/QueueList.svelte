<script lang="ts">
  // Contenu de la file d'attente (en cours, file manuelle, suite du contexte).
  // Partagé par le panneau File du shell et par la vue « En lecture ».
  import { commands } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import TrackRow from "./TrackRow.svelte";

  /** `current` : afficher aussi le titre en cours (inutile si la vue l'affiche déjà en grand). */
  let { current = true }: { current?: boolean } = $props();

  const empty = $derived(
    queue.view.manual.length === 0 && queue.view.upcoming.length === 0 && (!current || !player.state.track),
  );
</script>

<div class="queue-list">
  {#if current && player.state.track}
    <h3>En cours</h3>
    <div role="list">
      <TrackRow track={player.state.track} active compact actions={false} />
    </div>
  {/if}

  {#if queue.view.manual.length > 0}
    <h3>Ensuite dans la file</h3>
    <div role="list">
      {#each queue.view.manual as track, i (`${i}-${track.id}`)}
        <TrackRow {track} compact onRemove={() => commands.removeFromQueue(i)} />
      {/each}
    </div>
  {/if}

  {#if queue.view.upcoming.length > 0}
    <h3>Ensuite</h3>
    <div role="list">
      {#each queue.view.upcoming as track, i (`${i}-${track.id}`)}
        <TrackRow {track} compact actions={false} />
      {/each}
    </div>
  {/if}

  {#if empty}
    <p class="muted">La file est vide.</p>
  {/if}
</div>

<style>
  h3 {
    margin: 16px 10px 6px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }
</style>
