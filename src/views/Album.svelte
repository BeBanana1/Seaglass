<script lang="ts">
  import { commands, type AlbumPage, type AlbumRelated, type AlbumSummary, type MediaItem } from "../lib/ipc";
  import { albumKind, formatLongDuration, year } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { go } from "../lib/stores/nav.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import AlbumCredits from "../components/AlbumCredits.svelte";
  import CardGrid from "../components/CardGrid.svelte";

  /** `track` : titre à mettre en évidence (lien collé, #203). */
  let { id, track }: { id: number; track?: number } = $props();

  let page = $state<AlbumPage | null>(null);
  let error = $state<string | null>(null);
  /** Même album en hi-res chez l'artiste (#59), cherché après l'affichage de la page. */
  let twin = $state<AlbumSummary | null>(null);
  /** Autres versions et autres albums de l'artiste (#350), demandés après l'affichage. */
  let related = $state<AlbumRelated | null>(null);

  $effect(() => {
    const target = id;
    page = null;
    error = null;
    twin = null;
    related = null;
    commands
      .album(target)
      .then((p) => {
        if (target !== id) return;
        page = p;
        commands
          .albumRelated(target)
          .then((r) => target === id && (related = r))
          // Rangées facultatives : absentes en cas d'erreur (journalisée côté Rust).
          .catch((e) => console.warn("autres versions :", e));
        if (!p.album.hires) {
          commands
            .hiresTwinAlbum(target)
            .then((found) => target === id && (twin = found))
            // Simple suggestion : une erreur n'empêche rien, elle reste dans les journaux.
            .catch((e) => console.warn("album hi-res :", e));
        }
      })
      .catch((e) => (error = String(e)));
  });

  const asItems = (albums: AlbumSummary[]): MediaItem[] => albums.map((a) => ({ kind: "album", ...a }));
  // Le jumeau hi-res a déjà son bandeau (#59) : pas deux fois.
  const versions = $derived(related ? asItems(related.versions.filter((a) => a.id !== twin?.id)) : []);
  const byArtist = $derived(related ? asItems(related.byArtist) : []);
  const mainArtist = $derived(page?.album.artists[0] ?? null);
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    <p class="muted">{t.common.loading}</p>
  {:else}
    {@const album = page.album}
    {@const details = [
      year(album.releaseDate),
      t.common.tracks(page.tracks.length),
      formatLongDuration(page.duration),
    ].filter(Boolean)}
    <PageHeader
      image={page.coverLargeUrl ?? album.coverUrl}
      eyebrow={albumKind(album)}
      title={album.title}
      trackIds={page.tracks.map((track) => track.id)}
      source={{ kind: "album", id: album.id, title: album.title }}
      queueable
      favorite={{ kind: "album", id: album.id }}
      later={{ kind: "album", id: album.id }}
    >
      {#snippet meta()}
        {#each album.artists as artist, i (artist.id)}
          {#if i > 0},&nbsp;{/if}<button class="link" onclick={() => go({ name: "artist", id: artist.id })}>{artist.name}</button>
        {/each}
        <span class="details">{details.map((d) => ` · ${d}`).join("")}</span>
        {#if album.explicit}<span class="badge muted">E</span>{/if}
        {#if album.ai}<span class="badge muted">{t.common.ai}</span>{/if}
        {#if album.hires}<span class="badge">HI-RES</span>{/if}
      {/snippet}
    </PageHeader>
    {#if twin}
      {@const target = twin}
      <p class="hires-twin">
        <span class="badge">HI-RES</span>
        <span class="hires-twin-text">{t.views.album.hiresTwin}<span class="hires-twin-title">&nbsp;: {target.title}{target.releaseDate ? ` (${year(target.releaseDate)})` : ""}</span></span>
        <button class="hires-twin-open" onclick={() => go({ name: "album", id: target.id })}>{t.views.album.open}</button>
      </p>
    {/if}
    <TrackList
      tracks={page.tracks}
      source={{ kind: "album", id: page.album.id, title: page.album.title }}
      numbering="index"
      highlight={track}
      showAlbum={false}
      showCover={false}
    />
    <AlbumCredits albumId={page.album.id} tracks={page.tracks} />
    {#if page.copyright}<p class="copyright">{page.copyright}</p>{/if}
    <div class="album-related">
      <CardGrid title={t.views.album.versions} items={versions} row />
      {#if mainArtist}
        {@const artist = mainArtist}
        <CardGrid
          title={t.views.album.byArtist(artist.name)}
          items={byArtist}
          row
          more={{ label: t.views.album.seeArtist, onclick: () => go({ name: "artist", id: artist.id }) }}
        />
      {/if}
    </div>
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
    border-radius: var(--popover-radius);
    background: var(--bg-raised);
    font-size: 13px;
  }

  .hires-twin .badge {
    margin-left: 0;
  }

  .hires-twin-text {
    flex: 1;
    font-family: var(--meta-family);
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

  .album-related {
    margin-top: 32px;
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
