<script lang="ts">
  // Contenu de la file d'attente (en cours, file manuelle, suite du contexte).
  // Partagé par le panneau File du shell et par la vue « En lecture ».
  import { commands } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import TrackRow from "./TrackRow.svelte";

  /** `current` : afficher aussi le titre en cours (inutile si la vue l'affiche déjà en grand). */
  let { current = true }: { current?: boolean } = $props();

  const clearable = $derived(queue.view.manual.length > 0 || queue.view.upcoming.length > 0);

  async function clear() {
    try {
      await commands.clearQueue();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  const currentTrack = $derived(player.currentTrack);

  const empty = $derived(
    queue.view.manual.length === 0 && queue.view.upcoming.length === 0 && (!current || !currentTrack),
  );
</script>

<div class="queue-list">
  {#if clearable}
    <div class="queue-actions">
      <button class="queue-clear" onclick={clear} title={t.player.clearQueueHint}>
        {t.player.clearQueue}
      </button>
    </div>
  {/if}

  {#if current && currentTrack}
    <h3>{t.player.nowPlaying}</h3>
    <div role="list">
      <TrackRow track={currentTrack} active compact actions={false} />
    </div>
  {/if}

  {#if queue.view.manual.length > 0}
    <h3>{t.player.nextInQueue}</h3>
    <div role="list">
      {#each queue.view.manual as track, i (`${i}-${track.id}`)}
        <TrackRow {track} compact onRemove={() => commands.removeFromQueue(i)} />
      {/each}
    </div>
  {/if}

  {#if queue.view.upcoming.length > 0}
    <h3>{t.player.upNext}</h3>
    <div role="list">
      {#each queue.view.upcoming as track, i (`${i}-${track.id}`)}
        <TrackRow {track} compact actions={false} />
      {/each}
    </div>
  {/if}

  {#if empty}
    <p class="muted">{t.player.queueEmpty}</p>
  {/if}
</div>

<style>
  .queue-actions {
    display: flex;
    justify-content: flex-end;
    margin: 0 6px;
  }

  .queue-clear {
    padding: 4px 10px;
    border: 1px solid var(--border) !important;
    border-radius: 999px;
    font-size: 12px;
    color: var(--text-muted);
    background: var(--control-bg, none);
  }

  .queue-clear:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  :global([data-theme="studio"]) .queue-clear {
    border-radius: 0;
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
