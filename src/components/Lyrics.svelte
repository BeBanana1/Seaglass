<script lang="ts">
  // Paroles du titre en cours (#5). Horodatées : la ligne courante suit la position
  // interpolée (comme la barre de progression) et reste centrée ; clic = aller à la ligne.
  // Sans horodatage : texte simple.
  import { commands, type Lyrics, type TrackId } from "../lib/ipc";
  import { followPosition, player } from "../lib/stores/player.svelte";
  import { t } from "../lib/i18n.svelte";

  /** Délai pendant lequel le suivi automatique s'arrête après un défilement manuel. */
  const MANUAL_SCROLL_PAUSE_MS = 4000;

  let lyrics = $state<Lyrics | null>(null);
  let status = $state<"loading" | "ready" | "error">("loading");
  let active = $state(-1);
  let scroller = $state<HTMLElement>();
  let manualUntil = 0;

  const trackId = $derived(player.currentTrackId);
  const synced = $derived(lyrics?.text.kind === "synced" ? lyrics.text.lines : null);

  // Requête par titre ; une réponse arrivée après un changement de titre est ignorée.
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

  /** Dernière ligne commencée à `ms` (recherche dichotomique), -1 avant la première. */
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

  // Boucle d'images seulement pendant la lecture (#88) ; relancée quand les paroles arrivent.
  $effect(() => {
    const lines = synced;
    if (!lines) return;
    // 10 Hz sur Android : la ligne change au plus 100 ms après son horodatage.
    return followPosition((p) => {
      const next = lineAt(lines, p * 1000);
      if (next !== active) active = next;
    }, 10);
  });

  // Garde la ligne courante au centre, sauf si l'utilisateur fait défiler lui-même.
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

  // Molette, glissé tactile ou barre de défilement : défilement voulu par l'utilisateur
  // (un `scroll` seul ne suffit pas, le suivi automatique en émet aussi).
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
  /* Remplit le panneau (colonne flex) sous son titre. */
  .lyrics {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-height: 0;
    /* Unités `cqh` des marges de défilement : relatives à la hauteur du panneau. */
    container-type: size;
  }

  .lyrics-scroll {
    flex: 1;
    min-height: 0;
    overflow: auto;
    padding: 8px 8px 0;
    scrollbar-width: thin;
  }

  /* Marges pour que la première et la dernière ligne puissent venir au centre. */
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
