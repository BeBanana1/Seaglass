<script lang="ts">
  import { commands, type ArtistExtra, type ArtistPage, type MediaItem } from "../lib/ipc";
  import { go } from "../lib/stores/nav.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import { t } from "../lib/i18n.svelte";

  let { id }: { id: number } = $props();

  let page = $state<ArtistPage | null>(null);
  let error = $state<string | null>(null);
  // Extras (#205): requested together, shown as they arrive, with no visible error
  // (each part is simply absent).
  let extra = $state<ArtistExtra | null>(null);
  let bioOpen = $state(false);

  $effect(() => {
    const target = id;
    page = null;
    error = null;
    extra = null;
    bioOpen = false;
    commands
      .artist(target)
      .then((p) => target === id && (page = p))
      .catch((e) => (error = String(e)));
    commands
      .artistExtra(target)
      .then((x) => target === id && (extra = x))
      .catch(() => {});
  });

  const asItems = (albums: ArtistPage["albums"]): MediaItem[] =>
    albums.map((a) => ({ kind: "album", ...a }));
  const appearsOn = $derived(extra ? asItems(extra.appearsOn) : []);
  const similar = $derived<MediaItem[]>(
    extra ? extra.similar.map((a) => ({ kind: "artist", ...a })) : [],
  );
  // Collapse only if the text runs past about three lines.
  const bioLong = $derived(
    !!extra?.bio &&
      (extra.bio.paragraphs.length > 1 ||
        extra.bio.paragraphs.flat().reduce((n, part) => n + part.text.length, 0) > 280),
  );
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    <p class="muted">{t.common.loading}</p>
  {:else}
    <PageHeader
      image={page.artist.pictureUrl}
      eyebrow={t.format.artist}
      title={page.artist.name}
      round
      trackIds={page.topTracks.map((track) => track.id)}
      source={{ kind: "artist", id: page.artist.id, name: page.artist.name }}
      favorite={{ kind: "artist", id: page.artist.id }}
      onRadio={() => go({ name: "radio", seed: "artist", id })}
    >
      {#snippet meta()}
        <!-- Tracks where the artist is credited (#429), producer or composer included. -->
        <button class="artist-credits" onclick={() => go({ name: "contributor", id })}>
          {t.views.artist.credits}
        </button>
      {/snippet}
    </PageHeader>
    {#if page.topTracks.length > 0}
      <h2>{t.views.artist.popular}</h2>
      <TrackList
        tracks={page.topTracks}
        source={{ kind: "artist", id: page.artist.id, name: page.artist.name }}
        numbering="index"
      />
    {/if}
    <div class="grids">
      <CardGrid title={t.views.artist.albums} items={asItems(page.albums)} />
      <CardGrid title={t.views.artist.singles} items={asItems(page.singles)} />
      <CardGrid title={t.views.artist.appearsOn} items={appearsOn} />
    </div>
    {#if extra?.bio}
      <div class="artist-bio">
        <h2>{t.views.artist.about}</h2>
        <div class="bio-text" class:open={bioOpen || !bioLong}>
          {#each extra.bio.paragraphs as paragraph, i (i)}
            <p>
              {#each paragraph as part, j (j)}
                {#if part.kind === "text"}{part.text}{:else}<button
                    class="bio-link"
                    onclick={() =>
                      go(
                        part.kind === "artist"
                          ? { name: "artist", id: part.id }
                          : { name: "album", id: part.id },
                      )}
                  >
                    {part.text}
                  </button>{/if}
              {/each}
            </p>
          {/each}
        </div>
        <div class="bio-footer">
          {#if bioLong}
            <button class="bio-toggle" onclick={() => (bioOpen = !bioOpen)} aria-expanded={bioOpen}>
              {bioOpen ? t.views.artist.readLess : t.views.artist.readMore}
            </button>
          {/if}
          {#if extra.bio.source}<span class="bio-source">
              {t.views.artist.bioSource(extra.bio.source)}
            </span>{/if}
        </div>
      </div>
    {/if}
    <CardGrid title={t.views.artist.similar} items={similar} />
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  h2 {
    margin: 0 10px 8px;
    font-size: 17px;
    font-weight: 600;
  }

  .grids {
    margin-top: 28px;
  }

  .artist-bio {
    margin-bottom: 28px;
  }

  .bio-text {
    max-width: 760px;
    margin: 0 10px;
    line-height: 1.55;
    /* Collapsed: about three lines of the first paragraph, fading out at the bottom. */
    max-height: 4.65em;
    overflow: hidden;
    mask-image: linear-gradient(to bottom, #000 55%, transparent);
  }

  .bio-text.open {
    max-height: none;
    mask-image: none;
  }

  .bio-text p {
    margin: 0 0 0.8em;
  }

  .bio-link {
    padding: 0;
    font: inherit;
    color: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--accent) 60%, transparent);
    text-underline-offset: 2px;
  }

  .artist-credits {
    padding: 0;
    font: inherit;
    color: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--accent) 60%, transparent);
    text-underline-offset: 2px;
  }

  .artist-credits:hover,
  .bio-link:hover {
    color: var(--accent);
  }

  .bio-footer {
    display: flex;
    align-items: baseline;
    gap: 16px;
    margin: 8px 10px 0;
  }

  .bio-toggle {
    padding: 0;
    font-weight: 600;
    color: var(--accent);
  }

  .bio-source {
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
