<script lang="ts">
  // Audio chain: what the service sends → what the engine decodes (mpv, or Media3 on
  // Android) → what goes to the system output.
  import { player } from "../lib/stores/player.svelte";
  import {
    cacheLabel,
    engineName,
    formatStream,
    gainLabel,
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
    /** Full chain tightened onto one line (Studio theme player bar). */
    dense?: boolean;
  } = $props();

  const s = $derived(player.state);
  /** Format actually written to the output (#245), otherwise the decoding one (Android). */
  const sink = $derived(s.deviceFormat ?? s.output);
  const verdict = $derived(
    signalVerdict(s.stream, sink, s.volume, s.normalization, s.exclusive, s.equalizer),
  );
  const mode = $derived(s.exclusive ? t.player.exclusive : t.player.shared);
  /** Subtitle of the engine node: output format, and the volume if it modifies the signal. */
  const decodeSub = $derived(
    [
      s.output ? outputFormat(s.output.format) : "—",
      s.normalization !== null ? `RG ${gainLabel(s.normalization)}` : "",
      s.equalizer !== null ? t.player.equalizerShort(s.equalizer) : "",
      softwareGain(s.volume) ? t.player.volumeShort(volumeLabel(s.volume)) : "",
    ]
      .filter(Boolean)
      .join(" · "),
  );
  /** In-memory cache (#453, #498): position reached, or whole track, and the next
   *  preloaded one. Nothing until the engine has received anything. */
  const cacheSub = $derived.by(() => {
    const cache = s.track ? cacheLabel(s.cached, s.duration) : "";
    if (!cache) return "";
    return [cache, s.nextReady ? t.player.nextReady : ""].filter(Boolean).join(" · ");
  });
  /** Stream taken from another hi-res edition of the same recording (#18). */
  const edition = $derived(s.stream?.hiresEdition ?? null);
  const editionHint = $derived(edition ? t.player.editionHint(edition.title) : undefined);
  const sourceSub = $derived.by(() => {
    if (!s.stream) return "";
    const kind = edition
      ? t.player.hiresOtherEdition
      : s.stream.quality === "HI_RES_LOSSLESS"
        ? "Hi-Res"
        : s.stream.codec === "flac"
          ? "Lossless"
          : t.player.lossy;
    // Average bitrate from the manifest (#3); the tightened version keeps the line short.
    return s.stream.bitrate && !dense ? `${kind} · ${kbps(s.stream.bitrate)}` : kind;
  });
  /** Device actually opened (#79), as a fallback if the choice is unplugged. */
  const device = $derived(s.outputDevice);
  /** The tightened version keeps "WASAPI · shared": the name goes in the tooltip. */
  const outputSub = $derived(
    [
      (dense ? null : device?.name) ?? s.outputDriver?.toUpperCase() ?? "",
      s.outputDriver ? mode : "",
    ]
      .filter(Boolean)
      .join(" · "),
  );
  const outputTitle = $derived(
    [
      s.outputDriver ? `${s.outputDriver.toUpperCase()} · ${mode}` : "",
      device?.name
        ? device.systemDefault
          ? `${device.name} (${t.player.systemDevice})`
          : device.name
        : "",
      device?.missing ? t.player.deviceFallback(device.missing) : "",
    ]
      .filter(Boolean)
      .join(" — ") || undefined,
  );
  const sourceTitle = $derived(
    [editionHint, s.stream?.bitrate ? t.player.averageBitrate(kbps(s.stream.bitrate)) : ""]
      .filter(Boolean)
      .join(" — ") || undefined,
  );
</script>

{#if compact}
  <span
    class="chip {verdict.tone}"
    title="{verdict.label} — {verdict.detail}{editionHint ? ` — ${editionHint}` : ''}"
  >
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
      <span class="value mono">{engineName()}</span>
      <span class="sub">{decodeSub}</span>
      {#if cacheSub && !dense}
        <span class="cache mono" title={t.player.cacheHint}>{cacheSub}</span>
      {/if}
    </div>
    <span class="wire" aria-hidden="true"></span>
    <div class="node" title={outputTitle}>
      <span class="label">{t.player.output}</span>
      <span class="value mono">{sink ? `${khz(sink.samplerate)} kHz` : "—"}</span>
      <span class="sub output-device" class:device-fallback={!!device?.missing}>{outputSub}</span>
    </div>
    <div class="verdict" title={verdict.detail}>
      <span class="dot"></span>{verdict.label}
    </div>
    {#if edition && !dense}
      <p class="hires-edition" title={t.player.hiresEditionHint}>
        {t.player.hiresEdition} <span class="hires-edition-album">{edition.title}</span>
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

  /* In-memory cache (#453): an indented line, smaller than the others, out of the
     flow so as not to shift the Decoding node relative to its neighbors. */
  .node:has(.cache) {
    position: relative;
  }

  .path:has(.cache) {
    row-gap: 22px;
  }

  .cache {
    position: absolute;
    top: calc(100% + 2px);
    left: 0;
    font-size: 10px;
    color: var(--text-muted);
    opacity: 0.75;
    white-space: nowrap;
  }

  /* Windows device name, often long ("Speakers (… USB Audio)"). */
  .output-device {
    max-width: 240px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .device-fallback {
    color: var(--warn);
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

  /* Full-width line under the chain: edition actually played (#18). */
  .hires-edition {
    flex-basis: 100%;
    margin: 0;
    font-family: var(--meta-family);
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

  /* Android (#146, #158): the signal does not flow (animated `background-position` repaints
     the element on every frame). */
  :global(:root[data-platform="android"]) .wire {
    animation: none;
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
