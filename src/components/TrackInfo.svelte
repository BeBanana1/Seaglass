<script lang="ts">
  // « Infos du titre » (#3) : format réel servi par TIDAL (profondeur, fréquence, débit),
  // édition hi-res éventuelle, ReplayGain et métadonnées. Demandées au backend à
  // l'ouverture (une requête de lecture), jamais pour une liste entière.
  // Ouvert par `openTrackInfo` (lib/stores/trackInfo.svelte.ts).
  import { commands, type StreamFormat, type TrackInfo } from "../lib/ipc";
  import {
    artistNames,
    creditRole,
    decibels,
    formatStream,
    formatTime,
    fullTitle,
    kbps,
    khz,
    musicalKey,
    peak,
    tagLabel,
  } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import { closeTrackInfo, trackInfo } from "../lib/stores/trackInfo.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let info = $state<TrackInfo | null>(null);
  let error = $state<string | null>(null);
  let dialog = $state<HTMLDivElement>();
  let request = 0;

  $effect(() => {
    const track = trackInfo.track;
    if (!track) return;
    const id = ++request;
    info = null;
    error = null;
    commands
      .trackInfo(track.id)
      .then((i) => id === request && (info = i))
      .catch((e) => id === request && (error = String(e)));
    requestAnimationFrame(() => dialog?.focus());
  });

  const track = $derived(info?.track ?? trackInfo.track);
  const tags = $derived((info?.tags ?? []).map(tagLabel).filter((tag): tag is string => !!tag));
  const position = $derived.by(() => {
    if (!info?.trackNumber) return null;
    return info.volumeNumber && info.volumeNumber > 1
      ? t.info.trackAndDisc(info.trackNumber, info.volumeNumber)
      : t.info.trackNumber(info.trackNumber);
  });
  const hasGain = $derived(info && (info.trackGain !== null || info.albumGain !== null));

  function quality(f: StreamFormat): string {
    switch (f.quality) {
      case "HI_RES_LOSSLESS":
        return "Hi-Res Lossless";
      case "LOSSLESS":
        return "Lossless";
      case "HIGH":
        return "AAC 320";
      case "LOW":
        return "AAC 96";
    }
  }

  function resolution(f: StreamFormat): string | null {
    if (!f.bitDepth || !f.sampleRate) return f.sampleRate ? `${khz(f.sampleRate)} kHz` : null;
    return t.info.resolutionValue(f.bitDepth, khz(f.sampleRate));
  }

  function openAlbum(id: number) {
    closeTrackInfo();
    go({ name: "album", id });
  }

  function openArtist(id: number) {
    closeTrackInfo();
    go({ name: "artist", id });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      closeTrackInfo();
    }
  }
</script>

{#snippet format(f: StreamFormat)}
  <dt>{t.info.quality}</dt>
  <dd>{quality(f)}</dd>
  <dt>{t.info.format}</dt>
  <dd class="mono">{formatStream(f)}</dd>
  {#if resolution(f)}
    <dt>{t.info.resolution}</dt>
    <dd class="mono">{resolution(f)}</dd>
  {/if}
  {#if f.bitrate}
    <dt>{t.info.bitrate}</dt>
    <dd class="mono">{kbps(f.bitrate)}</dd>
  {/if}
{/snippet}

{#if trackInfo.track && track}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="info-backdrop" onclick={(e) => e.target === e.currentTarget && closeTrackInfo()}>
    <div
      class="track-info"
      role="dialog"
      aria-modal="true"
      aria-labelledby="track-info-title"
      tabindex="-1"
      onkeydown={onKeydown}
      bind:this={dialog}
    >
      <header>
        <span class="cover">
          {#if track.album.coverUrl}<img src={track.album.coverUrl} alt="" />{/if}
        </span>
        <div class="heading">
          <span class="label">{t.info.title}</span>
          <h3 id="track-info-title" class="display">{fullTitle(track)}</h3>
          <p class="sub">{artistNames(track)} · {track.album.title}</p>
        </div>
        <button class="close" onclick={closeTrackInfo} aria-label={t.common.close}><Icon name="close" /></button>
      </header>

      <div class="body" aria-busy={!info && !error}>
        {#if error}
          <p class="error">{error}</p>
        {:else if !info}
          <p class="sub">{t.common.loading}</p>
        {:else}
          <section class="info-section">
            <h4>{t.info.stream}</h4>
            {#if info.stream}
              <dl>{@render format(info.stream)}</dl>
            {:else}
              <p class="error">{info.streamError ?? t.info.streamUnavailable}</p>
            {/if}
            {#if tags.length > 0}
              <div class="info-tags" aria-label={t.info.availableIn}>
                {#each tags as tag (tag)}<span class="info-tag">{tag}</span>{/each}
              </div>
            {/if}
          </section>

          {#if info.hiresEdition}
            {@const edition = info.hiresEdition}
            <section class="info-section info-edition">
              <h4>{t.info.hiresEdition}</h4>
              <p class="note">
                {t.info.sameRecording}
                <button class="link" onclick={() => openAlbum(edition.track.album.id)}>{edition.track.album.title}</button>.
                {edition.played ? t.info.played : t.info.notPlayed}
              </p>
              {#if edition.stream}<dl>{@render format(edition.stream)}</dl>{/if}
            </section>
          {/if}

          {#if hasGain}
            <section class="info-section">
              <h4>{t.info.replayGain}</h4>
              <dl>
                {#if info.trackGain !== null}
                  <dt>{t.info.trackGain}</dt>
                  <dd class="mono">{decibels(info.trackGain)}</dd>
                {/if}
                {#if info.trackPeak !== null}
                  <dt>{t.info.trackPeak}</dt>
                  <dd class="mono">{peak(info.trackPeak)}</dd>
                {/if}
                {#if info.albumGain !== null}
                  <dt>{t.info.albumGain}</dt>
                  <dd class="mono">{decibels(info.albumGain)}</dd>
                {/if}
                {#if info.albumPeak !== null}
                  <dt>{t.info.albumPeak}</dt>
                  <dd class="mono">{peak(info.albumPeak)}</dd>
                {/if}
              </dl>
              <p class="note">{t.info.gainNote}</p>
            </section>
          {/if}

          <section class="info-section">
            <h4>{t.info.track}</h4>
            <dl>
              {#if position}
                <dt>{t.info.position}</dt>
                <dd>{position}</dd>
              {/if}
              <dt>{t.info.duration}</dt>
              <dd class="mono">{formatTime(track.duration)}</dd>
              {#if info.bpm}
                <dt>{t.info.tempo}</dt>
                <dd class="mono">{info.bpm} BPM</dd>
              {/if}
              {#if musicalKey(info.key, info.keyScale)}
                <dt>{t.info.key}</dt>
                <dd>{musicalKey(info.key, info.keyScale)}</dd>
              {/if}
              {#if track.isrc}
                <dt>ISRC</dt>
                <dd class="mono selectable">{track.isrc}</dd>
              {/if}
              {#if info.copyright}
                <dt>Copyright</dt>
                <dd>{info.copyright}</dd>
              {/if}
            </dl>
          </section>

          {#if info.credits.length > 0}
            <section class="info-section info-credits">
              <h4>{t.info.credits}</h4>
              <dl>
                {#each info.credits as group (group.role)}
                  <dt>{creditRole(group.role)}</dt>
                  <dd>
                    {#each group.contributors as person, i (person.name)}
                      {#if i > 0}{", "}{/if}{#if person.artistId !== null}
                        {@const id = person.artistId}
                        <button class="link credit-artist" onclick={() => openArtist(id)}>{person.name}</button>
                      {:else}{person.name}{/if}
                    {/each}
                  </dd>
                {/each}
              </dl>
            </section>
          {/if}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .info-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.45);
  }

  .track-info {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(480px, 100%);
    max-height: min(680px, 100%);
    padding: 18px;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
  }

  .track-info:focus {
    outline: none;
  }

  header {
    display: flex;
    align-items: flex-start;
    gap: 12px;
  }

  .cover {
    flex-shrink: 0;
    width: 56px;
    height: 56px;
    overflow: hidden;
    border-radius: 4px;
    background: var(--bg-raised);
  }

  .cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .heading {
    flex: 1;
    min-width: 0;
  }

  .label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  h3 {
    margin: 2px 0 0;
    font-size: 19px;
    overflow-wrap: anywhere;
  }

  .sub {
    margin: 2px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .close {
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .close:hover {
    color: var(--text);
  }

  .body {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-height: 120px;
    overflow-y: auto;
  }

  .info-section h4 {
    margin: 0 0 8px;
    font-family: var(--meta-family);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin: 0;
    font-size: 13px;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .selectable {
    user-select: text;
  }

  .info-tags {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 10px;
  }

  .info-tag {
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-family: var(--meta-family);
    font-size: 11px;
    color: var(--text-muted);
  }

  .note {
    margin: 0 0 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .info-section dl + .note {
    margin: 8px 0 0;
  }

  .credit-artist {
    padding: 0;
    font: inherit;
    color: var(--text);
    text-align: left;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--text) 35%, transparent);
    text-underline-offset: 2px;
  }

  .credit-artist:hover {
    text-decoration-color: currentColor;
  }

  .note .link {
    padding: 0;
    font: inherit;
    font-weight: 600;
    color: var(--text);
    text-align: left;
  }

  .note .link:hover {
    text-decoration: underline;
  }

  .error {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
  }

  :global([data-theme="studio"]) .info-tag,
  :global([data-theme="studio"]) .cover {
    border-radius: 0;
  }
</style>
