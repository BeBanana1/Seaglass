<script lang="ts">
  import { commands, type AlbumPage } from "../lib/ipc";
  import { albumKind, formatLongDuration, plural, year } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";

  let { id }: { id: number } = $props();

  let page = $state<AlbumPage | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const target = id;
    page = null;
    error = null;
    commands
      .album(target)
      .then((p) => target === id && (page = p))
      .catch((e) => (error = String(e)));
  });
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    <p class="muted">Chargement…</p>
  {:else}
    {@const album = page.album}
    {@const details = [
      year(album.releaseDate),
      plural(page.tracks.length, "titre"),
      formatLongDuration(page.duration),
    ].filter(Boolean)}
    <PageHeader
      image={page.coverLargeUrl ?? album.coverUrl}
      eyebrow={albumKind(album)}
      title={album.title}
      trackIds={page.tracks.map((t) => t.id)}
    >
      {#snippet meta()}
        {#each album.artists as artist, i (artist.id)}
          {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
        {/each}
        {#each details as part}
          <span> · {part}</span>
        {/each}
        {#if album.hires}<span class="badge">HI-RES</span>{/if}
      {/snippet}
    </PageHeader>
    <TrackList tracks={page.tracks} numbering="index" showAlbum={false} />
    {#if page.copyright}<p class="copyright">{page.copyright}</p>{/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .link {
    color: var(--text);
    font-weight: 500;
  }

  .link:hover {
    text-decoration: underline;
  }

  .badge {
    margin-left: 8px;
  }

  .copyright {
    margin: 20px 10px 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
