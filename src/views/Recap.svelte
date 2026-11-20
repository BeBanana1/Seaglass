<script lang="ts">
  // Rétrospective du mois et de l'année (#417) : tout est calculé par le backend (`recap`),
  // la page n'affiche que le résultat de la période choisie parmi celles qui ont des écoutes.
  import { commands, type MediaItem, type Recap, type RecapPeriod } from "../lib/ipc";
  import { go } from "../lib/stores/nav.svelte";
  import { coverLarge } from "../lib/format";
  import CardGrid from "../components/CardGrid.svelte";
  import TrackList from "../components/TrackList.svelte";
  import RecapBars from "../components/RecapBars.svelte";
  import RecapHeatmap from "../components/RecapHeatmap.svelte";
  import RecapMoments from "../components/RecapMoments.svelte";
  import RecapAudio from "../components/RecapAudio.svelte";
  import { t, num, locale } from "../lib/i18n.svelte";
  import { watchHistory } from "../lib/stores/handoff.svelte";

  type Mode = "month" | "year";

  let periods = $state<RecapPeriod[] | null>(null);
  let mode = $state<Mode>("month");
  // Index dans `shown`, du plus récent (0) au plus ancien.
  let index = $state(0);
  let recap = $state<Recap | null>(null);
  let error = $state<string | null>(null);

  /** Relu quand des écoutes arrivent de l'appareil lié (#430). */
  let version = $state(0);

  $effect(() => {
    void version;
    commands
      .recapPeriods()
      .then((p) => (periods = p))
      .catch((e) => (error = String(e)));
  });

  $effect(() => watchHistory(() => version++));

  const shown = $derived((periods ?? []).filter((p) => (mode === "year") === (p.month === null)));
  const period = $derived<RecapPeriod | undefined>(shown[index]);

  $effect(() => {
    const asked = period;
    void version;
    if (!asked) return;
    let stale = false;
    error = null;
    commands
      .recap(asked.year, asked.month)
      .then((r) => {
        if (!stale) recap = r;
      })
      .catch((e) => {
        if (!stale) error = String(e);
      });
    return () => (stale = true);
  });

  function setMode(next: Mode) {
    if (next === mode) return;
    // Passe au mois le plus récent de l'année affichée, ou à l'année du mois affiché.
    const year = period?.year;
    mode = next;
    recap = null;
    index = Math.max(
      0,
      shown.findIndex((p) => p.year === year),
    );
  }

  function step(delta: number) {
    const next = index + delta;
    if (next < 0 || next >= shown.length) return;
    recap = null;
    index = next;
  }

  function label(p: RecapPeriod): string {
    if (p.month === null) return t.library.recap.year(p.year);
    const text = new Date(p.year, p.month - 1, 1).toLocaleDateString(locale(), {
      month: "long",
      year: "numeric",
    });
    return text.charAt(0).toUpperCase() + text.slice(1);
  }

  const time = $derived.by(() => {
    const minutes = Math.round((recap?.seconds ?? 0) / 60);
    return t.library.stats.time(Math.floor(minutes / 60), minutes % 60);
  });
  const tracks = $derived(recap?.tracks.map((c) => c.item) ?? []);
  const trackNotes = $derived(recap?.tracks.map((c) => t.library.stats.plays(c.plays)) ?? []);
  const albums = $derived<MediaItem[]>(
    recap?.albums.map((c) => ({ kind: "album", ...c.item })) ?? [],
  );
  const artists = $derived<MediaItem[]>(
    recap?.artists.map((c) => ({ kind: "artist", ...c.item })) ?? [],
  );
  const newArtists = $derived<MediaItem[]>(
    recap?.newArtists.map((c) => ({ kind: "artist", ...c.item })) ?? [],
  );
  /** Timeline tick: every month of a year, every fifth day of a month. */
  function tick(i: number): string {
    if (period?.month !== null) return i % 5 === 0 ? String(i + 1) : "";
    return new Date(2000, i, 1).toLocaleDateString(locale(), { month: "narrow" });
  }

  function slot(i: number): string {
    if (period?.month === null)
      return new Date(period.year, i, 1).toLocaleDateString(locale(), { month: "long" });
    return period
      ? new Date(period.year, period.month - 1, i + 1).toLocaleDateString(locale())
      : "";
  }

  const topTrack = $derived(recap?.tracks[0]);
  const topArtist = $derived(recap?.artists[0]);
</script>

<section class="recap">
  <h1>{t.library.recap.title}</h1>

  {#if periods !== null && periods.length === 0}
    <p class="muted">{t.library.recap.empty}</p>
  {:else if periods !== null}
    <div class="recap-controls">
      <div class="tabs" role="tablist">
        {#each ["month", "year"] as const as id (id)}
          <button
            role="tab"
            aria-selected={id === mode}
            class:active={id === mode}
            onclick={() => setMode(id)}
          >
            {t.library.recap.modes[id]}
          </button>
        {/each}
      </div>
      <div class="recap-nav" role="group" aria-label={t.library.recap.periodLabel}>
        <button
          class="recap-step"
          onclick={() => step(1)}
          disabled={index >= shown.length - 1}
          aria-label={t.library.recap.previous}
        >
          ‹
        </button>
        <span class="recap-period">{period ? label(period) : ""}</span>
        <button
          class="recap-step"
          onclick={() => step(-1)}
          disabled={index <= 0}
          aria-label={t.library.recap.next}
        >
          ›
        </button>
      </div>
    </div>
  {/if}

  {#if error}
    <p class="error">{error}</p>
  {:else if recap && period}
    {#if recap.plays === 0}
      <p class="muted">{t.library.stats.empty}</p>
    {:else}
      <div class="recap-hero">
        <div class="recap-total">
          <span class="value">{time}</span>
          <span class="label">{t.library.stats.timeLabel}</span>
        </div>
        <div class="recap-total">
          <span class="value">{num(recap.plays)}</span>
          <span class="label">{t.library.stats.playsLabel}</span>
        </div>
        <div class="recap-total">
          <span class="value">{num(recap.days)}</span>
          <span class="label">{t.library.recap.daysLabel}</span>
        </div>
      </div>
      {#if recap.unknown > 0}<p class="hint">{t.library.stats.unknown(recap.unknown)}</p>{/if}

      <div class="recap-highlights">
        {#if topTrack}
          <button
            class="recap-highlight"
            onclick={() =>
              go({ name: "album", id: topTrack.item.album.id, track: topTrack.item.id })}
          >
            {#if coverLarge(topTrack.item.album)}<img
                src={coverLarge(topTrack.item.album)}
                alt=""
                loading="lazy"
              />{/if}
            <span class="recap-kicker">{t.library.recap.topTrack}</span>
            <span class="recap-name">{topTrack.item.title}</span>
            <span class="recap-sub">
              {topTrack.item.artists.map((a) => a.name).join(", ")} · {t.library.stats.plays(
                topTrack.plays,
              )}
            </span>
          </button>
        {/if}
        {#if topArtist}
          <button
            class="recap-highlight artist"
            onclick={() => go({ name: "artist", id: topArtist.item.id })}
          >
            {#if topArtist.item.pictureUrl}<img
                src={topArtist.item.pictureUrl}
                alt=""
                loading="lazy"
              />{/if}
            <span class="recap-kicker">{t.library.recap.topArtist}</span>
            <span class="recap-name">{topArtist.item.name}</span>
            <span class="recap-sub">{t.library.stats.plays(topArtist.plays)}</span>
          </button>
        {/if}
      </div>

      <div class="recap-panels">
        <div class="recap-panel">
          <h2>{t.library.recap.discoveries}</h2>
          <p class="recap-line">{t.library.recap.newArtists(recap.newArtistCount)}</p>
          <p class="recap-line">{t.library.recap.newTracks(recap.newTrackCount)}</p>
        </div>
        <div class="recap-panel">
          <h2>{t.library.recap.habits}</h2>
          {#if recap.topWeekday !== null}
            <p class="recap-line">
              {t.library.recap.topWeekday(t.library.recap.weekdays[recap.topWeekday])}
            </p>
          {/if}
          {#if recap.topHour !== null}<p class="recap-line">
              {t.library.recap.topHour(recap.topHour)}
            </p>{/if}
          <RecapBars
            values={recap.weekdays}
            labels={t.library.recap.weekdaysShort}
            titles={recap.weekdays.map(
              (n, i) => `${t.library.recap.weekdays[i]} · ${t.library.stats.plays(n)}`,
            )}
            top={recap.topWeekday}
            label={t.library.recap.weekdaysLabel}
          />
          <RecapBars
            values={recap.hours}
            labels={recap.hours.map((_, h) => (h % 6 === 0 ? String(h) : ""))}
            titles={recap.hours.map((n, h) => `${h} h · ${t.library.stats.plays(n)}`)}
            top={recap.topHour}
            label={t.library.recap.hoursLabel}
          />
        </div>
      </div>

      <div class="recap-panels">
        <div class="recap-panel">
          <h2>{t.library.recap.timeline}</h2>
          <RecapBars
            values={recap.timeline}
            labels={recap.timeline.map((_, i) => tick(i))}
            titles={recap.timeline.map((n, i) => `${slot(i)} · ${t.library.stats.plays(n)}`)}
            top={null}
            label={period.month === null
              ? t.library.recap.timelineMonths
              : t.library.recap.timelineDays}
          />
        </div>
        <div class="recap-panel">
          <h2>{t.library.recap.heatmap}</h2>
          <RecapHeatmap
            grid={recap.grid}
            days={t.library.recap.weekdaysShort}
            dayNames={t.library.recap.weekdays}
            label={t.library.recap.heatmapLabel}
            title={(day, hour, n) => `${day} ${hour} h · ${t.library.stats.plays(n)}`}
          />
        </div>
        <div class="recap-panel">
          <h2>{t.library.recap.moments}</h2>
          <RecapMoments {recap} />
        </div>
        {#if recap.audio}
          <div class="recap-panel">
            <h2>{t.library.recap.audio}</h2>
            <RecapAudio audio={recap.audio} plays={recap.plays} />
          </div>
        {/if}
      </div>

      {#if newArtists.length > 0}
        <div class="cards">
          <CardGrid
            title={t.library.recap.newArtistsTitle}
            items={newArtists}
            row
            notes={recap.newArtists.map((c) => t.library.stats.plays(c.plays))}
          />
        </div>
      {/if}

      {#if tracks.length > 0}
        <h2 class="section-title">{t.library.stats.tracks}</h2>
        <TrackList {tracks} numbering="index" notes={trackNotes} />
      {/if}
      <div class="cards">
        <CardGrid
          title={t.library.stats.albums}
          items={albums}
          row
          notes={recap.albums.map((c) => t.library.stats.plays(c.plays))}
        />
        <CardGrid
          title={t.library.stats.artists}
          items={artists}
          row
          notes={recap.artists.map((c) => t.library.stats.plays(c.plays))}
        />
      </div>
    {/if}
    <p class="hint">{t.library.recap.note}</p>
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  h1 {
    margin: 0 10px;
    font-size: 26px;
  }

  h2 {
    margin: 0;
    font-size: 17px;
    font-weight: 600;
  }

  .recap-controls {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    margin: 16px 10px 20px;
  }

  .tabs {
    display: flex;
    gap: 6px;
  }

  .tabs button {
    padding: 6px 14px;
    border-radius: 999px;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .tabs button.active {
    color: var(--bg);
    background: var(--text);
  }

  .recap-nav {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .recap-period {
    min-width: 150px;
    font-weight: 600;
    text-align: center;
  }

  .recap-step {
    width: 32px;
    height: 32px;
    border-radius: 50%;
    font-size: 20px;
    line-height: 1;
    background: var(--bg-raised);
  }

  .recap-step:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .recap-step:disabled {
    opacity: 0.35;
  }

  .recap-hero {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(96px, 1fr));
    gap: 12px;
    margin: 0 10px;
    padding: 20px;
    border: var(--panel-border, 1px solid var(--border));
    border-radius: var(--radius, 10px);
    background: var(--bg-raised);
    box-shadow: var(--panel-shadow, none);
  }

  .recap-total {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .recap-total .value {
    font-family: var(--font-display, inherit);
    font-size: 30px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .recap-total .label {
    font-size: 12px;
    color: var(--text-muted);
  }

  .recap-highlights,
  .recap-panels {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(260px, 1fr));
    gap: 12px;
    margin: 16px 10px 0;
  }

  .recap-highlight {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: flex-end;
    gap: 2px;
    min-height: 180px;
    padding: 16px;
    overflow: hidden;
    border-radius: var(--radius-card, 10px);
    color: #fff;
    background: var(--bg-raised);
    text-align: left;
  }

  .recap-highlight img {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    object-fit: cover;
    z-index: 0;
  }

  /* Voile sous le texte : lisible sur n'importe quelle pochette. */
  .recap-highlight::after {
    content: "";
    position: absolute;
    inset: 0;
    background: linear-gradient(to top, rgb(0 0 0 / 0.75), rgb(0 0 0 / 0.1) 70%);
    z-index: 0;
  }

  .recap-highlight > span {
    position: relative;
    z-index: 1;
  }

  .recap-kicker {
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    opacity: 0.85;
  }

  .recap-name {
    font-family: var(--font-display, inherit);
    font-size: 22px;
    font-weight: 700;
  }

  .recap-sub {
    font-size: 13px;
    opacity: 0.85;
  }

  .recap-panel {
    align-self: start;
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px;
    border: var(--panel-border, 1px solid var(--border));
    border-radius: var(--radius, 10px);
    background: var(--bg-raised);
  }

  .recap-line {
    margin: 0;
  }

  .section-title {
    margin: 28px 10px 10px;
  }

  .cards {
    margin-top: 28px;
  }

  .muted,
  .hint {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .hint {
    margin-top: 12px;
    font-size: 13px;
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }

  /* Téléphone : les trois totaux sur une ligne. */
  :global(:root[data-platform="android"]) .recap-hero {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    padding: 14px;
  }

  :global(:root[data-platform="android"]) .recap-total .value {
    font-size: 20px;
  }
</style>
