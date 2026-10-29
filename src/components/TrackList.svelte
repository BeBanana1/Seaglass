<script lang="ts">
  import { commands, type Track } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import TrackRow from "./TrackRow.svelte";

  let {
    tracks,
    numbering = "none",
    showAlbum = true,
    showCover = true,
  }: {
    /** Contexte de lecture : lire un titre met toute la liste en file. */
    tracks: Track[];
    numbering?: "none" | "index";
    showAlbum?: boolean;
    showCover?: boolean;
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
  {#each tracks as track, i (`${i}-${track.id}`)}
    <TrackRow
      {track}
      number={numbering === "index" ? i + 1 : undefined}
      compact={!showAlbum}
      {showCover}
      active={player.state.track?.id === track.id}
      onPlay={() => play(i)}
    />
  {/each}
</div>
