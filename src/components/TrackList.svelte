<script lang="ts">
  import { commands, type PlaySource, type Track } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import TrackRow from "./TrackRow.svelte";

  let {
    tracks,
    numbering = "none",
    positions,
    showAlbum = true,
    showCover = true,
    onRemoveFromPlaylist,
    source,
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
  } = $props();

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

<div role="list">
  {#each visible as track, i (`${positions?.[i] ?? i}-${track.id}`)}
    <TrackRow
      {track}
      number={numbering === "index" ? (positions?.[i] ?? i + 1) : undefined}
      compact={!showAlbum}
      {showCover}
      active={player.currentTrackId === track.id}
      onPlay={() => play(i)}
      onRemoveFromPlaylist={onRemoveFromPlaylist && (() => onRemoveFromPlaylist(i))}
    />
  {/each}
</div>
