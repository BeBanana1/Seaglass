<script lang="ts">
  // Contenu de la file d'attente (en cours, file manuelle, suite du contexte).
  // Partagé par le panneau File du shell et par la vue « En lecture ».
  import { commands } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import { fullTitle } from "../lib/format";
  import Icon from "./Icon.svelte";
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

  // Réorganisation de la file manuelle (#80) : glisser-déposer, Alt+↑ / Alt+↓, menu.
  // Pas d'affichage optimiste : la liste change au `queue-state` du backend.
  let manualList = $state<HTMLElement>();
  let dragFrom = $state<number | null>(null);
  /** Emplacement d'insertion pendant un glisser : avant la ligne `dropSlot` (= longueur : à la fin). */
  let dropSlot = $state<number | null>(null);
  /** Bouton à refocaliser dans la ligne déplacée au clavier, une fois la file reçue. */
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
    if (!e.altKey || (e.key !== "ArrowUp" && e.key !== "ArrowDown")) return;
    const to = e.key === "ArrowUp" ? i - 1 : i + 1;
    e.preventDefault();
    if (to < 0 || to >= queue.view.manual.length) return;
    const item = e.currentTarget as HTMLElement;
    const buttons = Array.from(item.querySelectorAll("button"));
    pendingFocus = { index: to, button: Math.max(0, buttons.indexOf(document.activeElement as HTMLButtonElement)) };
    move(i, to);
  }

  function onDragstart(e: DragEvent, i: number) {
    dragFrom = i;
    if (e.dataTransfer) {
      e.dataTransfer.effectAllowed = "move";
      e.dataTransfer.setData("text/plain", "");
    }
  }

  function onDragover(e: DragEvent, i: number) {
    if (dragFrom === null) return;
    e.preventDefault();
    if (e.dataTransfer) e.dataTransfer.dropEffect = "move";
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    dropSlot = e.clientY < rect.top + rect.height / 2 ? i : i + 1;
  }

  function onDrop(e: DragEvent) {
    e.preventDefault();
    if (dragFrom !== null && dropSlot !== null) {
      // L'entrée retirée décale d'un cran les emplacements qui la suivent.
      const to = dropSlot > dragFrom ? dropSlot - 1 : dropSlot;
      if (to !== dragFrom) move(dragFrom, to);
    }
    endDrag();
  }

  function onDragleave(e: DragEvent) {
    if (!manualList?.contains(e.relatedTarget as Node | null)) dropSlot = null;
  }

  function endDrag() {
    dragFrom = null;
    dropSlot = null;
  }

  /** Repère d'insertion, sauf là où le dépôt ne changerait rien. */
  function marker(slot: number): boolean {
    return dragFrom !== null && dropSlot === slot && slot !== dragFrom && slot !== dragFrom + 1;
  }

  const empty = $derived(
    queue.view.manual.length === 0 && queue.view.upcoming.length === 0 && (!current || !player.state.track),
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

  {#if current && player.state.track}
    <h3>{t.player.nowPlaying}</h3>
    <div role="list">
      <TrackRow track={player.state.track} active compact actions={false} />
    </div>
  {/if}

  {#if queue.view.manual.length > 0}
    <h3 title={t.player.reorderHint}>{t.player.nextInQueue}</h3>
    <div role="list" class="queue-manual" bind:this={manualList} ondragleave={onDragleave}>
      {#each queue.view.manual as track, i (`${i}-${track.id}`)}
        {@const last = queue.view.manual.length - 1}
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="queue-item"
          class:dragging={dragFrom === i}
          class:drop-before={marker(i)}
          class:drop-after={i === last && marker(i + 1)}
          draggable="true"
          ondragstart={(e) => onDragstart(e, i)}
          ondragover={(e) => onDragover(e, i)}
          ondrop={onDrop}
          ondragend={endDrag}
          onkeydown={(e) => onKeydown(e, i)}
        >
          <TrackRow
            {track}
            compact
            onRemove={() => commands.removeFromQueue(i)}
            onMoveUp={i > 0 ? () => move(i, i - 1) : undefined}
            onMoveDown={i < last ? () => move(i, i + 1) : undefined}
          />
        </div>
      {/each}
    </div>
  {/if}

  {#if queue.view.upcoming.length > 0}
    {#if queue.view.radio}
      <!-- La suite est une radio qui prolonge la file épuisée (#78). -->
      <h3 class="queue-radio">
        <Icon name="radio" size={14} />
        <span>{t.player.radioFrom(fullTitle(queue.view.radio))}</span>
      </h3>
    {:else}
      <h3>{t.player.upNext}</h3>
    {/if}
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

  .queue-item {
    position: relative;
    cursor: grab;
  }

  .queue-item :global(img) {
    -webkit-user-drag: none;
  }

  .queue-item.dragging {
    opacity: 0.4;
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

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }
</style>
