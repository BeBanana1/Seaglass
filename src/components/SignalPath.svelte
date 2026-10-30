<script lang="ts">
  // Chaîne audio : ce que le service envoie → ce que mpv décode → ce qui part vers Windows.
  import { player } from "../lib/stores/player.svelte";
  import {
    formatStream,
    kbps,
    khz,
    outputFormat,
    signalVerdict,
    softwareGain,
    volumeLabel,
  } from "../lib/format";
  import { t } from "../lib/i18n.svelte";

  let {
    compact = false,
    dense = false,
  }: {
    /** Pastille seule. */
    compact?: boolean;
    /** Chaîne complète resserrée sur une ligne (barre de lecture du thème Studio). */
    dense?: boolean;
  } = $props();

  const s = $derived(player.state);
  const verdict = $derived(signalVerdict(s.stream, s.output, s.volume));
  /** Sous-titre du nœud mpv : format de sortie, et le volume s'il modifie le signal. */
  const decodeSub = $derived(
    [s.output ? outputFormat(s.output.format) : "—", softwareGain(s.volume) ? t.player.volumeShort(volumeLabel(s.volume)) : ""]
      .filter(Boolean)
      .join(" · "),
  );
  /** Flux pris sur une autre édition hi-res du même enregistrement (#18). */
  const edition = $derived(s.stream?.hiresEdition ?? null);
  const editionHint = $derived(edition ? t.player.editionHint(edition) : undefined);
  const sourceSub = $derived.by(() => {
    if (!s.stream) return "";
    const kind = edition
      ? t.player.hiresOtherEdition
      : s.stream.quality === "HI_RES_LOSSLESS"
        ? "Hi-Res"
        : s.stream.codec === "flac"
          ? "Lossless"
          : t.player.lossy;
    // Débit moyen du manifeste (#3) ; la version resserrée garde la ligne courte.
    return s.stream.bitrate && !dense ? `${kind} · ${kbps(s.stream.bitrate)}` : kind;
  });
  const sourceTitle = $derived(
    [editionHint, s.stream?.bitrate ? t.player.averageBitrate(kbps(s.stream.bitrate)) : ""].filter(Boolean).join(" — ") ||
      undefined,
  );
</script>

{#if compact}
  <span class="chip {verdict.tone}" title="{verdict.label} — {verdict.detail}{editionHint ? ` — ${editionHint}` : ''}">
    <span class="dot"></span>
    {s.stream ? formatStream(s.stream) : "—"}
  </span>
{:else}
  <div class="path {verdict.tone}" class:dense>
    <div class="node" title={sourceTitle}>
      <span class="label">{t.player.source}</span>
      <span class="value mono">{s.stream ? formatStream(s.stream) : "—"}</span>
      <span class="sub">{sourceSub}</span>
    </div>
    <span class="wire" aria-hidden="true"></span>
    <div class="node">
      <span class="label">{t.player.decoding}</span>
      <span class="value mono">mpv</span>
      <span class="sub">{decodeSub}</span>
    </div>
    <span class="wire" aria-hidden="true"></span>
    <div class="node">
      <span class="label">{t.player.output}</span>
      <span class="value mono">{s.output ? `${khz(s.output.samplerate)} kHz` : "—"}</span>
      <span class="sub">{s.outputDriver ? `${s.outputDriver.toUpperCase()} · ${t.player.shared}` : ""}</span>
    </div>
    <div class="verdict" title={verdict.detail}>
      <span class="dot"></span>{verdict.label}
    </div>
    {#if edition && !dense}
      <p class="hires-edition" title={t.player.hiresEditionHint}>
        {t.player.hiresEdition} <span class="hires-edition-album">{edition}</span>
      </p>
    {/if}
  </div>
{/if}

<style>
  .ok {
    --tone: var(--ok);
  }
  .warn {
    --tone: var(--warn);
  }
  .lossy {
    --tone: var(--danger);
  }
  .idle {
    --tone: var(--text-muted);
  }

  .dot {
    display: inline-block;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--tone);
    box-shadow: 0 0 8px var(--tone);
  }

  .chip {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 3px 10px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-family: var(--font-mono);
    font-size: 11px;
    white-space: nowrap;
  }

  .path {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .node {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 92px;
  }

  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .value {
    font-size: 13px;
  }

  .sub {
    min-height: 1em;
    font-size: 11px;
    color: var(--text-muted);
  }

  .wire {
    flex: 1;
    min-width: 18px;
    max-width: 60px;
    height: 2px;
    background: linear-gradient(90deg, var(--border), var(--tone), var(--border));
    background-size: 200% 100%;
    animation: flow 2.4s linear infinite;
  }

  .verdict {
    display: inline-flex;
    align-items: center;
    gap: 8px;
    margin-left: auto;
    padding: 6px 12px;
    border: 1px solid color-mix(in oklab, var(--tone) 50%, transparent);
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    color: var(--tone);
    background: color-mix(in oklab, var(--tone) 10%, transparent);
  }

  /* Ligne pleine largeur sous la chaîne : édition réellement lue (#18). */
  .hires-edition {
    flex-basis: 100%;
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .hires-edition-album {
    font-weight: 600;
    color: var(--tone);
  }

  .dense {
    flex-wrap: nowrap;
    gap: 8px;
  }

  .dense .node {
    min-width: 0;
  }

  .dense .wire {
    min-width: 10px;
    max-width: 24px;
  }

  .dense .value {
    font-size: 12px;
    white-space: nowrap;
  }

  .dense .sub {
    white-space: nowrap;
  }

  .dense .verdict {
    padding: 0;
    border: none;
    background: none;
    font-size: 0;
  }

  @keyframes flow {
    to {
      background-position: -200% 0;
    }
  }

  :global([data-theme="studio"]) .chip,
  :global([data-theme="studio"]) .verdict {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .dot {
    box-shadow: none;
  }
</style>
