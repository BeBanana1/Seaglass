<script lang="ts">
  import { commands } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import TrackRow from "../components/TrackRow.svelte";
</script>

<aside>
  <h2>File d'attente</h2>

  {#if player.state.track}
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

  {#if !player.state.track && queue.view.manual.length === 0 && queue.view.upcoming.length === 0}
    <p class="muted">La file est vide.</p>
  {/if}
</aside>

<style>
  aside {
    height: 100%;
    overflow: auto;
    padding: 16px 12px;
    border-left: 1px solid var(--border);
    background: var(--bg);
  }

  h2 {
    margin: 0 8px 12px;
    font-size: 15px;
    font-weight: 600;
  }

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
