<script lang="ts">
  // Recap highlights (#521): longest streak, most replayed track in a day, first track of
  // the period. Everything comes ready-made from the backend (`recap`).
  import type { Recap, Track } from "../lib/ipc";
  import { go } from "../lib/stores/nav.svelte";
  import { artistNames, coverSmall } from "../lib/format";
  import { t, locale } from "../lib/i18n.svelte";

  let { recap }: { recap: Recap } = $props();

  /** `YYYY-MM-DD` (local day) as "14 March". */
  function day(date: string): string {
    const [y, m, d] = date.split("-").map(Number);
    return new Date(y, m - 1, d).toLocaleDateString(locale(), { day: "numeric", month: "long" });
  }

  function open(track: Track) {
    go({ name: "album", id: track.album.id, track: track.id });
  }
</script>

{#snippet row(track: Track, kicker: string, sub: string)}
  <button class="recap-moment" onclick={() => open(track)}>
    {#if coverSmall(track.album)}<img src={coverSmall(track.album)} alt="" loading="lazy" />{/if}
    <span class="recap-moment-text">
      <span class="recap-moment-kicker">{kicker}</span>
      <span class="recap-moment-title">{track.title}</span>
      <span class="recap-moment-sub">{sub}</span>
    </span>
  </button>
{/snippet}

{#if recap.streak > 0}<p class="recap-streak">{t.library.recap.streak(recap.streak)}</p>{/if}
{#if recap.topDay}
  {@render row(
    recap.topDay.track,
    t.library.recap.topDayKicker,
    t.library.recap.topDay(recap.topDay.plays, day(recap.topDay.date)),
  )}
{/if}
{#if recap.firstTrack}
  {@render row(recap.firstTrack, t.library.recap.firstTrack, artistNames(recap.firstTrack))}
{/if}

<style>
  .recap-streak {
    margin: 0;
  }

  .recap-moment {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 4px;
    padding: 6px;
    border-radius: var(--radius, 8px);
    text-align: left;
  }

  .recap-moment:hover {
    background: var(--bg-hover);
  }

  .recap-moment img {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 4px;
    object-fit: cover;
  }

  .recap-moment-text {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .recap-moment-text > span {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .recap-moment-kicker {
    font-size: 11px;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-muted);
  }

  .recap-moment-title {
    font-weight: 600;
  }

  .recap-moment-sub {
    font-size: 13px;
    color: var(--text-muted);
  }
</style>
