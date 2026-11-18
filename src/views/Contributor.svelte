<script lang="ts">
  import { commands, type ArtistSummary, type Contribution, type PlaySource } from "../lib/ipc";
  import { creditRole } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { go } from "../lib/stores/nav.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";

  // Titres où une personne est créditée (#429), par lots de 200 : le service en compte des
  // milliers pour un compositeur prolifique. Lire un titre met en contexte les titres chargés.
  let { id }: { id: number } = $props();

  let artist = $state<ArtistSummary | null>(null);
  let total = $state(0);
  let contributions = $state.raw<Contribution[]>([]);
  let nextOffset = $state<number | null>(null);
  let error = $state<string | null>(null);
  let loadingMore = $state(false);

  $effect(() => {
    const target = id;
    artist = null;
    contributions = [];
    nextOffset = null;
    error = null;
    commands
      .contributor(target, 0)
      .then((p) => {
        if (target !== id) return;
        artist = p.artist;
        total = p.total;
        contributions = p.tracks;
        nextOffset = p.nextOffset;
      })
      .catch((e) => (error = String(e)));
  });

  async function more() {
    if (nextOffset === null || loadingMore) return;
    const target = id;
    loadingMore = true;
    try {
      const p = await commands.contributor(target, nextOffset);
      if (target !== id) return;
      contributions = [...contributions, ...p.tracks];
      nextOffset = p.nextOffset;
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    } finally {
      loadingMore = false;
    }
  }

  const tracks = $derived(contributions.map((c) => c.track));
  const notes = $derived(contributions.map((c) => c.roles.map(creditRole).join(", ")));
  const source = $derived<PlaySource | undefined>(
    artist ? { kind: "contributor", id: artist.id, name: artist.name } : undefined,
  );
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !artist}
    <p class="muted">{t.common.loading}</p>
  {:else}
    <PageHeader
      image={artist.pictureUrl}
      eyebrow={t.views.contributor.eyebrow}
      title={artist.name}
      round
      trackIds={tracks.map((track) => track.id)}
      {source}
      favorite={{ kind: "artist", id: artist.id }}
      queueable
    >
      {#snippet meta()}
        {t.views.contributor.tracks(total)} ·
        <button class="contributor-artist" onclick={() => artist && go({ name: "artist", id: artist.id })}
          >{t.views.contributor.artistPage}</button
        >
      {/snippet}
    </PageHeader>
    {#if tracks.length === 0}
      <p class="muted">{t.views.contributor.empty}</p>
    {:else}
      <TrackList {tracks} {source} {notes} />
    {/if}
    {#if nextOffset !== null}
      <div class="contributor-more-wrap">
        <button class="contributor-more" onclick={more} disabled={loadingMore}>
          {loadingMore ? t.views.contributor.loading : t.views.contributor.more}
        </button>
      </div>
    {/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .contributor-artist {
    padding: 0;
    font: inherit;
    color: inherit;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--accent) 60%, transparent);
    text-underline-offset: 2px;
  }

  .contributor-artist:hover {
    color: var(--accent);
  }

  .contributor-more-wrap {
    display: flex;
    justify-content: center;
    margin-top: 16px;
  }

  .contributor-more {
    padding: 8px 18px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-weight: 600;
    color: var(--text);
  }

  .contributor-more:hover:not(:disabled) {
    color: var(--accent);
    border-color: var(--accent);
  }

  .contributor-more:disabled {
    opacity: 0.6;
  }

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
