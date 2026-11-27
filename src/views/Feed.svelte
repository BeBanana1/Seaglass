<script lang="ts" module>
  let cached: FeedItem[] | null = null;
  let cachedLang: string | null = null;
</script>

<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type FeedItem, type MediaItem } from "../lib/ipc";
  import { albumKind } from "../lib/format";
  import Card from "../components/Card.svelte";
  import { i18n, locale, t } from "../lib/i18n.svelte";
  import { feed } from "../lib/stores/feed.svelte";

  // Kept between two visits, like the home page: the feed only changes once a day.
  // Reloaded if the language changes (mix subtitles) or if there is something new (#415).
  if (cachedLang !== i18n.lang || feed.unseen > 0) cached = null;
  let items = $state<FeedItem[] | null>(cached);
  let error = $state<string | null>(null);

  const DAY_MS = 24 * 60 * 60 * 1000;

  /** Releases grouped: today, the last 7 days, then by month. */
  const groups = $derived.by(() => {
    const out: { label: string; items: FeedItem[] }[] = [];
    const today = new Date();
    today.setHours(0, 0, 0, 0);
    for (const item of items ?? []) {
      const label = groupLabel(item.occurredAt, today.getTime());
      const last = out[out.length - 1];
      if (last?.label === label) last.items.push(item);
      else out.push({ label, items: [item] });
    }
    return out;
  });

  const unseen = $derived((items ?? []).filter((i) => !i.seen).length);

  function groupLabel(at: number | null, today: number): string {
    if (at === null) return t.views.feed.earlier;
    if (at >= today) return t.views.feed.today;
    if (at >= today - 6 * DAY_MS) return t.views.feed.thisWeek;
    const month = new Date(at).toLocaleDateString(locale(), { month: "long", year: "numeric" });
    return month.charAt(0).toUpperCase() + month.slice(1);
  }

  /** Key of an item (an album and a mix can share a numeric id). */
  const itemKey = (item: MediaItem) =>
    `${item.kind}-${item.kind === "playlist" ? item.uuid : item.id}`;

  /** Type of the item: Single / EP / Album, or Mix for the monthly mix. */
  const itemKind = (item: MediaItem) => (item.kind === "album" ? albumKind(item) : t.format.mix);

  function shortDate(at: number | null): string | null {
    return at === null
      ? null
      : new Date(at).toLocaleDateString(locale(), { day: "numeric", month: "short" });
  }

  onMount(async () => {
    if (items) return;
    try {
      const loaded = await commands.feed();
      items = loaded;
      cachedLang = i18n.lang;
      // Opening the page counts as reading (#415): the "New" badges stay shown
      // during this visit, not the next one.
      cached = loaded.map((entry) => ({ ...entry, seen: true }));
      if (loaded.some((entry) => !entry.seen)) {
        commands.markFeedSeen().catch((e) => console.warn("feed not marked as seen", e));
      }
    } catch (e) {
      error = String(e);
    }
  });
</script>

<section>
  <div class="feed-head">
    <h1>{t.views.feed.title}</h1>
    <p class="muted">
      {t.views.feed.intro}{#if unseen > 0}&nbsp;· <span class="feed-count">
          {t.views.feed.unseen(unseen)}
        </span>{/if}
    </p>
  </div>
  {#if error}
    <p class="error">{error}</p>
  {:else if !items}
    <p class="muted">{t.common.loading}</p>
  {:else if items.length === 0}
    <p class="muted">{t.views.feed.empty}</p>
  {:else}
    {#each groups as group (group.label)}
      <section class="feed-group">
        <h2>{group.label}</h2>
        <div class="items">
          {#each group.items as entry, i (`${itemKey(entry.item)}-${i}`)}
            <div class="feed-item">
              <Card item={entry.item} />
              {#if !entry.seen}<span class="feed-new">{t.views.feed.new}</span>{/if}
              <span class="feed-meta">
                {[itemKind(entry.item), shortDate(entry.occurredAt)].filter(Boolean).join(" · ")}
              </span>
            </div>
          {/each}
        </div>
      </section>
    {/each}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  section.feed-group {
    padding: 0;
    margin-bottom: 28px;
  }

  .feed-head {
    margin: 0 10px 20px;
  }

  h1 {
    margin: 0 0 4px;
    font-size: 26px;
  }

  .feed-head .muted {
    margin: 0;
  }

  .feed-count {
    color: var(--accent);
    font-weight: 600;
  }

  h2 {
    margin: 0 10px 8px;
    font-size: 17px;
    font-weight: 600;
  }

  .items {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(170px, 1fr));
  }

  .feed-item {
    position: relative;
    display: flex;
    flex-direction: column;
  }

  .feed-new {
    position: absolute;
    top: 18px;
    left: 18px;
    padding: 2px 8px;
    border-radius: var(--radius-pill);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: var(--accent-contrast);
    background: var(--accent);
    box-shadow: var(--shadow-raised);
    pointer-events: none;
  }

  .feed-meta {
    margin: -6px 10px 6px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }

  /* Android (#24) : deux cartes par ligne, comme `CardGrid`. */
  :global(:root[data-platform="android"]) .items {
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  }
</style>
