<script lang="ts">
  import { commands, type PlaylistPage } from "../lib/ipc";
  import { formatLongDuration, playlistSubtitle } from "../lib/format";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";

  let { uuid }: { uuid: string } = $props();

  let page = $state<PlaylistPage | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const target = uuid;
    page = null;
    error = null;
    commands
      .playlist(target)
      .then((p) => target === uuid && (page = p))
      .catch((e) => (error = String(e)));
  });
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    <p class="muted">Chargement…</p>
  {:else}
    {@const playlist = page.playlist}
    <PageHeader
      image={playlist.imageUrl}
      eyebrow="Playlist"
      title={playlist.title}
      description={playlist.description}
      trackIds={page.tracks.map((t) => t.id)}
      queueable
    >
      {#snippet meta()}
        {playlistSubtitle(playlist)}
        {#if formatLongDuration(playlist.duration)} · {formatLongDuration(playlist.duration)}{/if}
      {/snippet}
    </PageHeader>
    <TrackList tracks={page.tracks} numbering="index" />
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
