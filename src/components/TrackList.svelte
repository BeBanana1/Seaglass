<script lang="ts">
  import { commands, type PlaySource, type Track } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import TrackRow from "./TrackRow.svelte";
  import { tick, untrack } from "svelte";
  import { keyboardMove, Reorder } from "../lib/reorder.svelte";
  import { platform } from "../lib/platform.svelte";
  import { selection, type SelectionList } from "../lib/stores/selection.svelte";
  import { selectionMenu } from "../lib/selectionActions";
  import type { TrackId } from "../lib/ipc";

  let {
    tracks,
    numbering = "none",
    positions,
    showAlbum = true,
    showCover = true,
    onRemoveFromPlaylist,
    source,
    highlight,
    onMove,
    notes,
    onRemoveSelected,
    context,
  }: {
    /** Playback context: playing a track queues the whole list, in this order. */
    tracks: Track[];
    /** Origin of the list, shown in "Played from" (#124). */
    source?: PlaySource;
    numbering?: "none" | "index";
    /** Numbers to display (original position of a sorted or filtered list), otherwise 1, 2, 3… */
    positions?: number[];
    showAlbum?: boolean;
    showCover?: boolean;
    /** List of one of your playlists: "Remove from this playlist" on every track. */
    onRemoveFromPlaylist?: (index: number) => void;
    /** Track highlighted and brought into view (pasted link, #203). */
    highlight?: number;
    /**
     * Reorderable list (your playlist, neither sorted nor filtered, #201): drag and drop,
     * Alt+↑ / Alt+↓, "Move up / Move down". `to` = final position; `true` if the
     * move took place.
     */
    onMove?: (from: number, to: number) => Promise<boolean>;
    /** Note specific to each row (`TrackRow` › `note`), in the order of `tracks`. */
    notes?: string[];
    /** One of your playlists: "Remove from the playlist" on multiple selection (#332). */
    onRemoveSelected?: (ids: ReadonlySet<TrackId>) => Promise<void>;
    /**
     * Context of a click on track `index` when the displayed list is only a part
     * (filter, #496); without it, the displayed list.
     */
    context?: (index: number) => { ids: TrackId[]; start: number };
  } = $props();

  // Multiple selection (#332): the list registers while it is mounted; leaving it
  // (page change) empties its selection.
  const handle: SelectionList = {
    tracks: () => tracks,
    get removeFromPlaylist() {
      return onRemoveSelected;
    },
  };

  $effect(() => {
    selection.register(handle);
    return () => selection.unregister(handle);
  });

  // Filter or reload: tracks that are no longer displayed leave the selection.
  $effect(() => {
    const ids = tracks.map((t) => t.id);
    untrack(() => selection.prune(handle, ids));
  });

  // Only changes at the start and end of a selection: the rows are not redrawn on
  // every checked track.
  const selecting = $derived(selection.owner === handle && selection.count > 0);

  function select(id: TrackId, how: "toggle" | "extend") {
    if (how === "extend") selection.extend(handle, id);
    else selection.toggle(handle, id);
  }

  let list = $state<HTMLElement>();
  const reorder = new Reorder((from, to) => move(from, to));

  async function move(from: number, to: number, focusButton?: number) {
    if (!onMove || !(await onMove(from, to)) || focusButton === undefined) return;
    // The focus follows the row moved with the keyboard.
    await tick();
    list?.children[to]?.querySelectorAll<HTMLElement>("button")[focusButton]?.focus();
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    const to = keyboardMove(e, i, tracks.length);
    if (to === null) return;
    const buttons = Array.from((e.currentTarget as HTMLElement).querySelectorAll("button"));
    move(i, to, Math.max(0, buttons.indexOf(document.activeElement as HTMLButtonElement)));
  }

  // Chunked rendering (#181): a long list created all its rows in one block, and the
  // page stayed empty until then (nearly a second for 500 tracks on the phone). The
  // first rows go out right away, the rest follows task after task, which leaves the
  // browser paint between two chunks. Playback keeps the whole list as its context.
  const FIRST_ROWS = 40;
  const ROWS_PER_STEP = 80;
  let rendered = $state(FIRST_ROWS);
  const visible = $derived(tracks.length > rendered ? tracks.slice(0, rendered) : tracks);

  $effect(() => {
    if (rendered >= tracks.length) return;
    const timer = setTimeout(() => (rendered += ROWS_PER_STEP), 0);
    return () => clearTimeout(timer);
  });

  async function play(index: number) {
    const { ids, start } = context?.(index) ?? { ids: tracks.map((t) => t.id), start: index };
    try {
      // A track picked in a list keeps the current shuffle mode (#526).
      await commands.playTracks(ids, start, null, source ?? null);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

{#snippet row(track: Track, i: number)}
  <TrackRow
    {track}
    number={numbering === "index" ? (positions?.[i] ?? i + 1) : undefined}
    compact={!showAlbum}
    {showCover}
    active={player.currentTrackId === track.id}
    highlighted={highlight === track.id}
    laterButton={source?.kind === "later"}
    note={notes?.[i]}
    selected={selection.isSelected(handle, track.id)}
    {selecting}
    onSelect={(how) => select(track.id, how)}
    {selectionMenu}
    onPlay={() => play(i)}
    onRemoveFromPlaylist={onRemoveFromPlaylist && (() => onRemoveFromPlaylist(i))}
    onMoveUp={onMove && i > 0 ? () => move(i, i - 1) : undefined}
    onMoveDown={onMove && i < tracks.length - 1 ? () => move(i, i + 1) : undefined}
  />
{/snippet}

<div role="list" bind:this={list} ondragleave={onMove && ((e) => reorder.dragleave(e, list))}>
  {#each visible as track, i (`${positions?.[i] ?? i}-${track.id}`)}
    {#if onMove}
      <!-- svelte-ignore a11y_no_static_element_interactions -->
      <div
        class="track-item"
        class:dragging={reorder.from === i}
        class:drop-before={reorder.marker(i)}
        class:drop-after={i === visible.length - 1 && reorder.marker(i + 1)}
        draggable={platform.mobile ? undefined : "true"}
        ondragstart={(e) => reorder.dragstart(e, i)}
        ondragover={(e) => reorder.dragover(e, i)}
        ondrop={(e) => reorder.drop(e)}
        ondragend={() => reorder.end()}
        onkeydown={(e) => onKeydown(e, i)}
      >
        {@render row(track, i)}
      </div>
    {:else}
      {@render row(track, i)}
    {/if}
  {/each}
</div>

<style>
  .track-item {
    position: relative;
    cursor: grab;
  }

  .track-item :global(img) {
    -webkit-user-drag: none;
  }

  .track-item.dragging {
    opacity: 0.4;
  }

  .track-item.drop-before::before,
  .track-item.drop-after::after {
    content: "";
    position: absolute;
    z-index: 1;
    left: 6px;
    right: 6px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
    pointer-events: none;
  }

  .track-item.drop-before::before {
    top: -1px;
  }

  .track-item.drop-after::after {
    bottom: -1px;
  }

  /* Android: no HTML5 drag (it would get in the way of the long press), "Move up / Move
     down" go through the menu. */
  :global(:root[data-platform="android"]) .track-item {
    cursor: auto;
  }
</style>
