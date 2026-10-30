<script lang="ts">
  // « Infos du titre » (#3) : format réel servi par TIDAL (profondeur, fréquence, débit),
  // édition hi-res éventuelle, ReplayGain et métadonnées. Demandées au backend à
  // l'ouverture (une requête de lecture), jamais pour une liste entière.
  // Ouvert par `openTrackInfo` (lib/stores/trackInfo.svelte.ts).
  import { commands, type StreamFormat, type TrackInfo } from "../lib/ipc";
  import {
    artistNames,
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
  const tags = $derived((info?.tags ?? []).map(tagLabel).filter((t): t is string => !!t));
  const position = $derived.by(() => {
    if (!info?.trackNumber) return null;
    return info.volumeNumber && info.volumeNumber > 1
      ? `Piste ${info.trackNumber}, disque ${info.volumeNumber}`
      : `Piste ${info.trackNumber}`;
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
    return `${f.bitDepth} bits / ${khz(f.sampleRate)} kHz`;
  }

  function openAlbum(id: number) {
    closeTrackInfo();
    go({ name: "album", id });
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      closeTrackInfo();
    }
  }
</script>

{#snippet format(f: StreamFormat)}
  <dt>Qualité</dt>
  <dd>{quality(f)}</dd>
  <dt>Format</dt>
  <dd class="mono">{formatStream(f)}</dd>
  {#if resolution(f)}
    <dt>Résolution</dt>
    <dd class="mono">{resolution(f)}</dd>
  {/if}
  {#if f.bitrate}
    <dt>Débit moyen</dt>
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
          <span class="label">Infos du titre</span>
          <h3 id="track-info-title" class="display">{fullTitle(track)}</h3>
          <p class="sub">{artistNames(track)} · {track.album.title}</p>
        </div>
        <button class="close" onclick={closeTrackInfo} aria-label="Fermer"><Icon name="close" /></button>
      </header>

      <div class="body" aria-busy={!info && !error}>
        {#if error}
          <p class="error">{error}</p>
        {:else if !info}
          <p class="sub">Chargement…</p>
        {:else}
          <section class="info-section">
            <h4>Flux</h4>
            {#if info.stream}
              <dl>{@render format(info.stream)}</dl>
            {:else}
              <p class="error">{info.streamError ?? "Flux indisponible"}</p>
            {/if}
            {#if tags.length > 0}
              <div class="info-tags" aria-label="Disponible en">
                {#each tags as tag (tag)}<span class="info-tag">{tag}</span>{/each}
              </div>
            {/if}
          </section>

          {#if info.hiresEdition}
            {@const edition = info.hiresEdition}
            <section class="info-section info-edition">
              <h4>Édition hi-res</h4>
              <p class="note">
                Même enregistrement sur
                <button class="link" onclick={() => openAlbum(edition.track.album.id)}>{edition.track.album.title}</button>.
                {edition.played
                  ? "Seaglass la lit à la place de ce titre."
                  : "Pas lue à sa place (Réglages › Lecture › Qualité)."}
              </p>
              {#if edition.stream}<dl>{@render format(edition.stream)}</dl>{/if}
            </section>
          {/if}

          {#if hasGain}
            <section class="info-section">
              <h4>Normalisation (ReplayGain)</h4>
              <dl>
                {#if info.trackGain !== null}
                  <dt>Gain du titre</dt>
                  <dd class="mono">{decibels(info.trackGain)}</dd>
                {/if}
                {#if info.trackPeak !== null}
                  <dt>Crête du titre</dt>
                  <dd class="mono">{peak(info.trackPeak)}</dd>
                {/if}
                {#if info.albumGain !== null}
                  <dt>Gain de l'album</dt>
                  <dd class="mono">{decibels(info.albumGain)}</dd>
                {/if}
                {#if info.albumPeak !== null}
                  <dt>Crête de l'album</dt>
                  <dd class="mono">{peak(info.albumPeak)}</dd>
                {/if}
              </dl>
              <p class="note">Indicatif : Seaglass n'applique aucun gain, pour rester bit-perfect.</p>
            </section>
          {/if}

          <section class="info-section">
            <h4>Titre</h4>
            <dl>
              {#if position}
                <dt>Position</dt>
                <dd>{position}</dd>
              {/if}
              <dt>Durée</dt>
              <dd class="mono">{formatTime(track.duration)}</dd>
              {#if info.bpm}
                <dt>Tempo</dt>
                <dd class="mono">{info.bpm} BPM</dd>
              {/if}
              {#if musicalKey(info.key, info.keyScale)}
                <dt>Tonalité</dt>
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
    border: 1px solid var(--border);
    border-radius: calc(var(--panel-radius) + 2px);
    color: var(--text);
    background: var(--dialog-bg);
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.4);
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

  :global([data-theme="studio"]) .track-info,
  :global([data-theme="studio"]) .info-tag,
  :global([data-theme="studio"]) .cover {
    border-radius: 0;
  }
</style>
