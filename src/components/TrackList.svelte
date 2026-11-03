<script lang="ts">
  import { commands, type Track } from "../lib/ipc";
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
  }: {
    /** Contexte de lecture : lire un titre met toute la liste en file, dans cet ordre. */
    tracks: Track[];
    numbering?: "none" | "index";
    /** Numéros à afficher (position d'origine d'une liste triée ou filtrée), sinon 1, 2, 3… */
    positions?: number[];
    showAlbum?: boolean;
    showCover?: boolean;
    /** Liste d'une de vos playlists : « Retirer de cette playlist » sur chaque titre. */
    onRemoveFromPlaylist?: (index: number) => void;
  } = $props();

  async function play(index: number) {
    try {
      await commands.playTracks(
        tracks.map((t) => t.id),
        index,
      );
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<div role="list">
  {#each tracks as track, i (`${positions?.[i] ?? i}-${track.id}`)}
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
