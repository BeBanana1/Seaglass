<script lang="ts">
  import { commands, type RadioSeed, type Track } from "../lib/ipc";
  import { formatLongDuration, plural } from "../lib/format";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";

  /** Un mix du service, ou la radio d'un titre ou d'un artiste : même présentation. */
  type Source = { mix: string } | { seed: RadioSeed; id: number };

  interface View {
    eyebrow: string;
    title: string;
    description: string | null;
    image: string | null;
    round: boolean;
    tracks: Track[];
  }

  let { source }: { source: Source } = $props();

  let view = $state<View | null>(null);
  let error = $state<string | null>(null);

  async function load(s: Source): Promise<View> {
    if ("mix" in s) {
      const p = await commands.mix(s.mix);
      return {
        eyebrow: "Mix",
        title: p.mix.title,
        description: p.mix.subtitle,
        image: p.imageLargeUrl ?? p.mix.imageUrl,
        round: false,
        tracks: p.tracks,
      };
    }
    const p = await commands.radio(s.seed, s.id);
    return {
      eyebrow: "Radio",
      title: p.title,
      description: s.seed === "track" ? "Des titres proches de ce morceau." : "Des titres proches de cet artiste.",
      image: p.imageUrl,
      round: s.seed === "artist",
      tracks: p.tracks,
    };
  }

  $effect(() => {
    const target = source;
    view = null;
    error = null;
    load(target)
      .then((v) => target === source && (view = v))
      .catch((e) => (error = String(e)));
  });

  const duration = $derived(view ? view.tracks.reduce((sum, t) => sum + t.duration, 0) : 0);
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !view}
    <p class="muted">Chargement…</p>
  {:else}
    <PageHeader
      image={view.image}
      eyebrow={view.eyebrow}
      title={view.title}
      description={view.description}
      round={view.round}
      trackIds={view.tracks.map((t) => t.id)}
      queueable
    >
      {#snippet meta()}
        {plural(view?.tracks.length ?? 0, "titre")}
        {#if formatLongDuration(duration)} · {formatLongDuration(duration)}{/if}
      {/snippet}
    </PageHeader>
    {#if view.tracks.length === 0}
      <p class="muted">Aucun titre disponible.</p>
    {:else}
      <TrackList tracks={view.tracks} numbering="index" />
    {/if}
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
