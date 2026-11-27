<script lang="ts">
  import { commands, type PlaySource, type RadioSeed, type Track } from "../lib/ipc";
  import { formatLongDuration } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import { platform } from "../lib/platform.svelte";

  /** A mix from the service, or the radio of a track or an artist: same presentation. */
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
        eyebrow: t.format.mix,
        title: p.mix.title,
        description: p.mix.subtitle,
        image: p.imageLargeUrl ?? p.mix.imageUrl,
        round: false,
        tracks: p.tracks,
      };
    }
    const p = await commands.radio(s.seed, s.id);
    return {
      eyebrow: t.views.mix.radio,
      title: p.title,
      description: s.seed === "track" ? t.views.mix.trackRadio : t.views.mix.artistRadio,
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

  const playSource = $derived<PlaySource | undefined>(
    !view
      ? undefined
      : "mix" in source
        ? { kind: "mix", id: source.mix, title: view.title }
        : { kind: "radio", seed: source.seed, id: source.id, title: view.title },
  );

  const duration = $derived(view ? view.tracks.reduce((sum, track) => sum + track.duration, 0) : 0);
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !view}
    <p class="muted">{t.common.loading}</p>
  {:else}
    <PageHeader
      image={view.image}
      eyebrow={view.eyebrow}
      title={view.title}
      description={view.description}
      round={view.round}
      trackIds={view.tracks.map((track) => track.id)}
      source={playSource}
      favorite={"mix" in source ? { kind: "mix", id: source.mix } : undefined}
      pin={"mix" in source ? { kind: "mix", id: source.mix, title: view.title } : undefined}
      queueable
    >
      {#snippet meta()}
        {t.common.tracks(view?.tracks.length ?? 0)}
        {#if formatLongDuration(duration)}
          · {formatLongDuration(duration)}{/if}
      {/snippet}
    </PageHeader>
    {#if view.tracks.length === 0}
      <p class="muted">{t.views.mix.empty}</p>
    {:else}
      <!-- Android: no numbers, they clutter more than they help (#162). -->
      <TrackList
        tracks={view.tracks}
        source={playSource}
        numbering={platform.mobile ? "none" : "index"}
      />
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
