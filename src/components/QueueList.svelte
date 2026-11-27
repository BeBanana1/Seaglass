<script lang="ts">
  // Content of the queue (now playing, manual queue, rest of the context).
  // Shared by the shell's Queue panel and by the "Now playing" view.
  import { commands, type QueuePart, type Track } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { pushToast, pushUndoToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import { fullTitle } from "../lib/format";
  import Icon from "./Icon.svelte";
  import TrackRow from "./TrackRow.svelte";
  import { eggs } from "../lib/eggs/eggs.svelte";
  import { keyboardMove, Reorder } from "../lib/reorder.svelte";
  import { touchReorder } from "../lib/touch-reorder";
  import { platform } from "../lib/platform.svelte";
  import { guestOf } from "../lib/stores/jam.svelte";
  import { openPlaylistPicker } from "../lib/stores/playlistPicker.svelte";

  /** `current`: also show the current track (useless if the view already shows it large). */
  let { current = true }: { current?: boolean } = $props();

  const clearable = $derived(queue.view.manual.length > 0 || queue.view.upcoming.length > 0);

  /** The whole queue, not only the displayed tracks (#327). */
  async function saveAsPlaylist() {
    try {
      const trackIds = await commands.queueTrackIds();
      if (trackIds.length === 0) return;
      openPlaylistPicker({
        trackIds,
        label: `${t.player.queue} · ${t.common.tracks(trackIds.length)}`,
      });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  /** Undoes the last clear or removal (#349), if the queue has not changed since. */
  async function undo() {
    try {
      if (!(await commands.restoreQueue()))
        pushToast({ level: "info", message: t.player.undoGone });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function clear() {
    try {
      await commands.clearQueue();
      pushUndoToast(t.player.queueCleared, undo);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function remove(index: number) {
    try {
      await commands.removeFromQueue(index);
      pushUndoToast(t.player.removedFromQueue, undo);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  const currentTrack = $derived(player.currentTrack);

  /** Plays a track of the queue (#167): those before it are skipped. */
  async function play(part: QueuePart, index: number, track: Track) {
    try {
      await commands.playFromQueue(part, index, track.id);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  // Reordering the manual queue (#80): drag and drop, Alt+↑ / Alt+↓, menu.
  // Android (#450): HTML5 dragging does not work with a finger and a long press opens the menu
  // of the track; a handle on the right of the row grabs it right away (`touchReorder`).
  // No optimistic display: the list changes on the backend's `queue-state`.
  let manualList = $state<HTMLElement>();
  const reorder = new Reorder((from, to) => move(from, to));
  const manualItems = () =>
    Array.from(manualList?.querySelectorAll<HTMLElement>(".queue-item") ?? []);
  /** Button to refocus in the row moved with the keyboard, once the queue is received. */
  let pendingFocus: { index: number; button: number } | null = null;

  async function move(from: number, to: number) {
    try {
      await commands.moveInQueue(from, to);
    } catch (e) {
      pendingFocus = null;
      pushToast({ level: "error", message: String(e) });
    }
  }

  $effect(() => {
    void queue.view.manual;
    if (!pendingFocus || !manualList) return;
    const { index, button } = pendingFocus;
    pendingFocus = null;
    manualList.children[index]?.querySelectorAll<HTMLElement>("button")[button]?.focus();
  });

  function onKeydown(e: KeyboardEvent, i: number) {
    const to = keyboardMove(e, i, queue.view.manual.length);
    if (to === null) return;
    const item = e.currentTarget as HTMLElement;
    const buttons = Array.from(item.querySelectorAll("button"));
    pendingFocus = {
      index: to,
      button: Math.max(0, buttons.indexOf(document.activeElement as HTMLButtonElement)),
    };
    move(i, to);
  }

  const empty = $derived(
    queue.view.manual.length === 0 &&
      queue.view.upcoming.length === 0 &&
      (!current || !currentTrack),
  );
</script>

<div class="queue-list">
  {#if clearable}
    <div class="queue-actions">
      <button
        class="queue-clear queue-save"
        onclick={saveAsPlaylist}
        title={t.player.saveQueueHint}
      >
        {t.player.saveQueue}
      </button>
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
    <h3 title={t.player.reorderHint}>{t.player.nextInQueue}</h3>
    <div
      role="list"
      class="queue-manual"
      bind:this={manualList}
      ondragleave={(e) => reorder.dragleave(e, manualList)}
    >
      {#each queue.view.manual as track, i (`${i}-${track.id}`)}
        {@const last = queue.view.manual.length - 1}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="queue-item"
          class:touch={platform.mobile}
          class:dragging={reorder.from === i}
          class:drop-before={reorder.marker(i)}
          class:drop-after={i === last && reorder.marker(i + 1)}
          draggable={platform.mobile ? undefined : "true"}
          ondragstart={(e) => reorder.dragstart(e, i)}
          ondragover={(e) => reorder.dragover(e, i)}
          ondrop={(e) => reorder.drop(e)}
          ondragend={() => reorder.end()}
          onkeydown={(e) => onKeydown(e, i)}
          use:touchReorder={{
            enabled: platform.mobile,
            index: i,
            reorder,
            items: manualItems,
            handle: ".queue-handle",
          }}
        >
          <TrackRow
            {track}
            compact
            onPlay={() => play("manual", i, track)}
            onRemove={() => remove(i)}
            onMoveUp={i > 0 ? () => move(i, i - 1) : undefined}
            onMoveDown={i < last ? () => move(i, i + 1) : undefined}
            addedBy={guestOf(track.id)}
          />
          {#if platform.mobile}
            <!-- For keyboard and screen readers: Alt+↑ / Alt+↓ and the menu's Move up / Down. -->
            <span class="queue-handle" aria-hidden="true"><Icon name="grip" size={18} /></span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}

  {#if queue.view.upcoming.length > 0}
    {#if queue.view.radio}
      <!-- The rest is a radio that extends the exhausted queue (#78). -->
      <h3 class="queue-radio">
        <Icon name="radio" size={14} />
        <span>{t.player.radioFrom(fullTitle(queue.view.radio))}</span>
      </h3>
    {:else}
      <h3>{t.player.upNext}</h3>
    {/if}
    <div role="list">
      {#each queue.view.upcoming as track, i (`${i}-${track.id}`)}
        <TrackRow {track} compact actions={false} onPlay={() => play("upcoming", i, track)} />
      {/each}
      <!-- #399: on February 29th, a fake "hidden track" closes the queue, like on a disc.
           Neither clickable, nor read by screen readers. -->
      {#if eggs.date === "leapDay"}
        <div class="hidden-track" aria-hidden="true">
          <span class="hidden-title">{t.eggs.hiddenTrack}</span>
          <span class="hidden-time mono">–:––</span>
        </div>
      {/if}
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
    gap: 6px;
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

  .queue-item {
    position: relative;
    cursor: grab;
  }

  .queue-item :global(img) {
    -webkit-user-drag: none;
  }

  .queue-item.dragging:not(:global(.touch-dragging)) {
    opacity: 0.4;
  }

  /* Android (#450): the row and its handle side by side. */
  .queue-item.touch {
    display: flex;
    align-items: center;
    cursor: default;
  }

  .queue-item.touch > :global(.row) {
    flex: 1;
    min-width: 0;
  }

  .queue-handle {
    display: grid;
    flex-shrink: 0;
    place-items: center;
    align-self: stretch;
    width: 36px;
    color: var(--text-muted);
    /* A finger placed here moves the row, never the page. */
    touch-action: none;
  }

  /* Row grabbed by finger: it follows the finger, above the others (like `HomeEditor`). */
  .queue-item:global(.touch-dragging) {
    z-index: 1;
    border-radius: var(--radius);
    transform: translateY(var(--drag-y, 0px)) scale(1.02);
    background: linear-gradient(var(--bg-raised), var(--bg-raised)), var(--bg);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }

  .queue-item:global(.touch-dragging) .queue-handle {
    color: var(--accent);
  }

  .queue-item.drop-before::before,
  .queue-item.drop-after::after {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
    pointer-events: none;
  }

  .queue-item.drop-before::before {
    top: -1px;
  }

  .queue-item.drop-after::after {
    bottom: -1px;
  }

  h3 {
    margin: 16px 10px 6px;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .queue-radio {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-top: 12px;
    border-top: 1px solid var(--border);
    color: var(--accent);
  }

  .queue-radio span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .queue-radio :global(svg) {
    flex-shrink: 0;
  }

  .hidden-track {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    margin: 2px 6px;
    padding: 8px 10px;
    border: 1px dashed var(--border);
    border-radius: 6px;
    font-size: 13px;
    font-style: italic;
    color: var(--text-muted);
    opacity: 0.7;
    cursor: default;
    user-select: none;
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }
</style>
