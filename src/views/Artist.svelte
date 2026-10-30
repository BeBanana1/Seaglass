<script lang="ts">
  import { commands, type ArtistPage, type MediaItem } from "../lib/ipc";
  import { go } from "../lib/stores/nav.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import { t } from "../lib/i18n.svelte";

  let { id }: { id: number } = $props();

  let page = $state<ArtistPage | null>(null);
  let error = $state<string | null>(null);

  $effect(() => {
    const target = id;
    page = null;
    error = null;
    commands
      .artist(target)
      .then((p) => target === id && (page = p))
      .catch((e) => (error = String(e)));
  });

  const asItems = (albums: ArtistPage["albums"]): MediaItem[] =>
    albums.map((a) => ({ kind: "album", ...a }));
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
      favorite={{ kind: "artist", id: page.artist.id }}
      onRadio={() => go({ name: "radio", seed: "artist", id })}
    />
    {#if page.topTracks.length > 0}
      <h2>{t.views.artist.popular}</h2>
      <TrackList tracks={page.topTracks} numbering="index" />
    {/if}
    <div class="grids">
      <CardGrid title={t.views.artist.albums} items={asItems(page.albums)} />
      <CardGrid title={t.views.artist.singles} items={asItems(page.singles)} />
    </div>
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

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
