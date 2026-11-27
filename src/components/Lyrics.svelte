<script lang="ts">
  // Lyrics of the current track (#5). Timestamped: the current line follows the
  // interpolated position (like the progress bar) and stays centered; click = go to the line.
  // Without timestamps: plain text.
  import { commands, type Lyrics, type TrackId } from "../lib/ipc";
  import { followPosition, player } from "../lib/stores/player.svelte";
  import { t } from "../lib/i18n.svelte";

  /** Time during which automatic following pauses after a manual scroll. */
  const MANUAL_SCROLL_PAUSE_MS = 4000;

  let lyrics = $state<Lyrics | null>(null);
  let status = $state<"loading" | "ready" | "error">("loading");
  let active = $state(-1);
  let scroller = $state<HTMLElement>();
  let manualUntil = 0;

  const trackId = $derived(player.currentTrackId);
  const synced = $derived(lyrics?.text.kind === "synced" ? lyrics.text.lines : null);

  // One request per track; a response that arrives after a track change is ignored.
  let requested: TrackId | null = null;
  $effect(() => {
    const id = trackId;
    requested = id;
    lyrics = null;
    active = -1;
    manualUntil = 0;
    if (id === null) return;
    status = "loading";
    commands.lyrics(id).then(
      (l) => {
        if (requested !== id) return;
        lyrics = l;
        status = "ready";
      },
      () => {
        if (requested === id) status = "error";
      },
    );
  });

  /** Last line started at `ms` (binary search), -1 before the first. */
  function lineAt(lines: { timeMs: number }[], ms: number): number {
    let lo = 0;
    let hi = lines.length - 1;
    let found = -1;
    while (lo <= hi) {
      const mid = (lo + hi) >> 1;
      if (lines[mid].timeMs <= ms) {
        found = mid;
        lo = mid + 1;
      } else hi = mid - 1;
    }
    return found;
  }

  // Frame loop only during playback (#88); restarted when the lyrics arrive.
  $effect(() => {
    const lines = synced;
    if (!lines) return;
    // 10 Hz on Android: the line changes at most 100 ms after its timestamp.
    return followPosition((p) => {
      const next = lineAt(lines, p * 1000);
      if (next !== active) active = next;
    }, 10);
  });

  // Keeps the current line centered, unless the user scrolls by themselves.
  $effect(() => {
    const i = active;
    if (!scroller || i < 0 || performance.now() < manualUntil) return;
    const line = scroller.querySelector<HTMLElement>(`[data-line="${i}"]`);
    if (!line) return;
    const box = scroller.getBoundingClientRect();
    const at = line.getBoundingClientRect();
    const top = scroller.scrollTop + at.top - box.top - (box.height - at.height) / 2;
    scroller.scrollTo({ top, behavior: "smooth" });
  });

  function userScrolled() {
    manualUntil = performance.now() + MANUAL_SCROLL_PAUSE_MS;
  }

  // Wheel, touch drag or scrollbar: scrolling wanted by the user
  // (a `scroll` alone is not enough, automatic following emits them too).
  $effect(() => {
    const el = scroller;
    if (!el) return;
    const events = ["wheel", "touchmove", "pointerdown"] as const;
    for (const e of events) el.addEventListener(e, userScrolled, { passive: true });
    return () => {
      for (const e of events) el.removeEventListener(e, userScrolled);
    };
  });

  function seek(ms: number) {
    manualUntil = 0;
    void commands.seek(ms / 1000);
  }

  const providerName = (p: string) =>
    p === "MUSIXMATCH" ? "Musixmatch" : p.charAt(0) + p.slice(1).toLowerCase();
</script>

<div class="lyrics" dir={lyrics?.rtl ? "rtl" : undefined}>
  {#if trackId === null}
    <p class="lyrics-empty">{t.player.lyricsNothing}</p>
  {:else if status === "loading"}
    <p class="lyrics-empty">{t.player.lyricsLoading}</p>
  {:else if status === "error"}
    <p class="lyrics-empty">{t.player.lyricsError}</p>
  {:else if !lyrics}
    <p class="lyrics-empty">{t.player.lyricsNone}</p>
  {:else}
    <div class="lyrics-scroll" class:synced={!!synced} bind:this={scroller}>
      {#if synced}
        {#each synced as line, i (i)}
          <button
            class="lyrics-line"
            class:active={i === active}
            class:past={i < active}
            class:gap={!line.text}
            data-line={i}
            onclick={() => seek(line.timeMs)}
          >
            {line.text || "♪"}
          </button>
        {/each}
      {:else if lyrics.text.kind === "plain"}
        {#each lyrics.text.lines as line, i (i)}
          <p class="lyrics-plain" class:gap={!line}>{line}</p>
        {/each}
      {/if}
    </div>
    {#if lyrics.provider}
      <p class="lyrics-credit">{t.player.lyricsCredit(providerName(lyrics.provider))}</p>
    {/if}
  {/if}
</div>

<style>
  /* Fills the panel (flex column) under its title. */
  .lyrics {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    /* `cqh` units of the scroll margins: relative to the panel height. */
    container-type: size;
  }

  .lyrics-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 8px 0;
    scrollbar-width: thin;
  }

  /* Margins so the first and last lines can reach the center. */
  .lyrics-scroll.synced {
    padding-block: 45cqh;
    mask-image: linear-gradient(transparent, #000 14%, #000 86%, transparent);
  }

  .lyrics-line {
    display: block;
    width: 100%;
    padding: 6px 8px;
    border-radius: 8px;
    font-family: var(--display-family);
    font-size: 20px;
    font-weight: 600;
    line-height: 1.35;
    text-align: start;
    color: var(--text-muted);
    opacity: 0.45;
    transition:
      opacity 0.3s,
      color 0.3s,
      transform 0.3s;
    transform-origin: left center;
  }

  .lyrics[dir="rtl"] .lyrics-line {
    transform-origin: right center;
  }

  .lyrics-line.past {
    opacity: 0.3;
  }

  .lyrics-line.active {
    color: var(--text);
    opacity: 1;
    transform: scale(1.04);
  }

  .lyrics-line:hover {
    opacity: 0.9;
    background: var(--bg-hover);
  }

  .lyrics-line.gap {
    font-size: 16px;
  }

  .lyrics-plain {
    margin: 0;
    padding: 2px 8px;
    font-size: 16px;
    line-height: 1.5;
  }

  .lyrics-plain.gap {
    height: 14px;
  }

  .lyrics-empty {
    margin: 24px 8px;
    color: var(--text-muted);
  }

  .lyrics-credit {
    margin: 8px 8px 0;
    font-size: 11px;
    color: var(--text-muted);
  }

  :global([data-theme="studio"]) .lyrics-line {
    border-radius: 0;
    font-size: 18px;
  }

  :global([data-theme="atelier"]) .lyrics-line {
    font-weight: 500;
  }
</style>
