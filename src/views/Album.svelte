<script lang="ts">
  import { commands, type AlbumPage, type AlbumSummary } from "../lib/ipc";
  import { albumKind, formatLongDuration, plural, year } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";

  let { id }: { id: number } = $props();

  let page = $state<AlbumPage | null>(null);
  let error = $state<string | null>(null);
  /** Même album en hi-res chez l'artiste (#59), cherché après l'affichage de la page. */
  let twin = $state<AlbumSummary | null>(null);

  $effect(() => {
    const target = id;
    page = null;
    error = null;
    twin = null;
    commands
      .album(target)
      .then((p) => {
        if (target !== id) return;
        page = p;
        if (!p.album.hires) {
          commands
            .hiresTwinAlbum(target)
            .then((t) => target === id && (twin = t))
            // Simple suggestion : une erreur n'empêche rien, elle reste dans les journaux.
            .catch((e) => console.warn("album hi-res :", e));
        }
      })
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
      queueable
    >
      {#snippet meta()}
        {#each album.artists as artist, i (artist.id)}
          {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
        {/each}
        <span class="details">{details.map((d) => ` · ${d}`).join("")}</span>
        {#if album.hires}<span class="badge">HI-RES</span>{/if}
      {/snippet}
    </PageHeader>
    {#if twin}
      {@const target = twin}
      <p class="hires-twin">
        <span class="badge">HI-RES</span>
        <span class="hires-twin-text">Cet album existe en Hi-Res<span class="hires-twin-title">&nbsp;: {target.title}{target.releaseDate ? ` (${year(target.releaseDate)})` : ""}</span></span>
        <button class="hires-twin-open" onclick={() => go({ name: "album", id: target.id })}>Ouvrir</button>
      </p>
    {/if}
    <TrackList tracks={page.tracks} numbering="index" showAlbum={false} showCover={false} />
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

  .hires-twin {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 0 10px 16px;
    padding: 10px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
    font-size: 13px;
  }

  .hires-twin .badge {
    margin-left: 0;
  }

  .hires-twin-text {
    flex: 1;
    min-width: 0;
  }

  .hires-twin-title {
    color: var(--text-muted);
  }

  .hires-twin-open {
    padding: 5px 14px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .hires-twin-open:hover {
    background: var(--accent-strong);
  }

  :global([data-theme="studio"]) .hires-twin,
  :global([data-theme="studio"]) .hires-twin-open,
  :global([data-theme="atelier"]) .hires-twin,
  :global([data-theme="atelier"]) .hires-twin-open {
    border-radius: 0;
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
