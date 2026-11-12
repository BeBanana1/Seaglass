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
  }: {
    /** Contexte de lecture : lire un titre met toute la liste en file, dans cet ordre. */
    tracks: Track[];
    /** Origine de la liste, affichée dans « Joué depuis » (#124). */
    source?: PlaySource;
    numbering?: "none" | "index";
    /** Numéros à afficher (position d'origine d'une liste triée ou filtrée), sinon 1, 2, 3… */
    positions?: number[];
    showAlbum?: boolean;
    showCover?: boolean;
    /** Liste d'une de vos playlists : « Retirer de cette playlist » sur chaque titre. */
    onRemoveFromPlaylist?: (index: number) => void;
    /** Titre mis en évidence et amené à l'écran (lien collé, #203). */
    highlight?: number;
    /**
     * Liste réorganisable (votre playlist, ni triée ni filtrée, #201) : glisser-déposer,
     * Alt+↑ / Alt+↓, « Monter / Descendre ». `to` = position finale ; `true` si le
     * déplacement a eu lieu.
     */
    onMove?: (from: number, to: number) => Promise<boolean>;
    /** Mention propre à chaque ligne (`TrackRow` › `note`), dans l'ordre de `tracks`. */
    notes?: string[];
    /** Une de vos playlists : « Retirer de la playlist » sur la sélection multiple (#332). */
    onRemoveSelected?: (ids: ReadonlySet<TrackId>) => Promise<void>;
  } = $props();

  // Sélection multiple (#332) : la liste s'inscrit tant qu'elle est montée ; la quitter
  // (changement de page) vide sa sélection.
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

  // Filtre ou rechargement : les titres qui ne sont plus affichés sortent de la sélection.
  $effect(() => {
    const ids = tracks.map((t) => t.id);
    untrack(() => selection.prune(handle, ids));
  });

  // Ne change qu'au début et à la fin d'une sélection : les lignes ne se redessinent pas à
  // chaque titre coché.
  const selecting = $derived(selection.owner === handle && selection.count > 0);

  function select(id: TrackId, how: "toggle" | "extend") {
    if (how === "extend") selection.extend(handle, id);
    else selection.toggle(handle, id);
  }

  let list = $state<HTMLElement>();
  const reorder = new Reorder((from, to) => move(from, to));

  async function move(from: number, to: number, focusButton?: number) {
    if (!onMove || !(await onMove(from, to)) || focusButton === undefined) return;
    // Le focus suit la ligne déplacée au clavier.
    await tick();
    list?.children[to]?.querySelectorAll<HTMLElement>("button")[focusButton]?.focus();
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    const to = keyboardMove(e, i, tracks.length);
    if (to === null) return;
    const buttons = Array.from((e.currentTarget as HTMLElement).querySelectorAll("button"));
    move(i, to, Math.max(0, buttons.indexOf(document.activeElement as HTMLButtonElement)));
  }

  // Rendu par morceaux (#181) : une longue liste créait toutes ses lignes d'un bloc, et la
  // page restait vide jusque-là (près d'une seconde pour 500 titres sur le téléphone). Les
  // premières lignes partent aussitôt, le reste suit tâche après tâche, ce qui laisse le
  // navigateur peindre entre deux morceaux. La lecture garde toute la liste en contexte.
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
    try {
      await commands.playTracks(
        tracks.map((t) => t.id),
        index,
        false,
        source ?? null,
      );
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

  /* Android : pas de glisser HTML5 (il gênerait l'appui long), « Monter / Descendre » passent
     par le menu. */
  :global(:root[data-platform="android"]) .track-item {
    cursor: auto;
  }
</style>
