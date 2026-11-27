<script lang="ts">
  import { commands } from "../lib/ipc";
  import { followPosition, player } from "../lib/stores/player.svelte";
  import { formatTime } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { eggs } from "../lib/eggs/eggs.svelte";
  import { stationClock } from "../lib/eggs/playback";

  const ENDING_SECONDS = 30;

  let { large = false }: { large?: boolean } = $props();

  let position = $state(0);
  /** Value being dragged: the bar no longer follows playback while it is held. */
  let dragging = $state<number | null>(null);

  // Frame loop only during playback (#88). On Android, once per second
  // (#473): the displayed time only changes every second, and the "Now playing" bar advances
  // by one or two pixels per second. The themes style `.fill` by its width (gradients,
  // Cassette reels): no compositor animation here, unlike the
  // mini-lecteur.
  $effect(() => followPosition((p) => (position = p), 1));

  const s = $derived(player.state);
  const duration = $derived(s.duration);
  const shown = $derived(dragging ?? position);
  /** August 1st (#399): the bar follows the second hand of a station clock, stop included.
   *  Not while dragging, nor in the time displayed as digits. */
  const bar = $derived(
    dragging === null && eggs.date === "swissDay" ? stationClock(position, duration) : shown,
  );
  const ratio = $derived(duration ? Math.min(bar / duration, 1) : 0);
  /** Part already in memory (#453), as a lighter band under the progress. */
  const cachedRatio = $derived(s.track && duration ? Math.min(s.cached / duration, 1) : 0);
  /** The time text only changes every second: no recomputation on every frame. */
  const shownText = $derived(formatTime(Math.floor(shown)));
  /** Last seconds of the track, when the next one is preloaded (same 30 s threshold as
   *  the actor): a simple marker the themes can show (Session, #339). */
  const ending = $derived(duration > ENDING_SECONDS && duration - shown <= ENDING_SECONDS);

  async function seekTo(value: number) {
    dragging = null;
    await commands.seek(value);
  }
</script>

<div class="progress" class:large class:ending>
  <span class="time mono">{shownText}</span>
  <div class="track" style:--ratio={ratio} style:--cached={cachedRatio}>
    <div class="cached"></div>
    <div class="fill"></div>
    <input
      type="range"
      min="0"
      max={s.duration || 1}
      step="0.1"
      value={bar}
      disabled={!s.track || !s.duration}
      aria-label={t.player.position}
      oninput={(e) => (dragging = e.currentTarget.valueAsNumber)}
      onchange={(e) => seekTo(e.currentTarget.valueAsNumber)}
    />
  </div>
  <span class="time mono">{formatTime(s.duration)}</span>
</div>

<style>
  .progress {
    display: flex;
    align-items: center;
    gap: 10px;
    width: 100%;
  }

  .time {
    min-width: 42px;
    font-size: 11px;
    color: var(--text-muted);
    text-align: center;
  }

  .track {
    position: relative;
    flex: 1;
    height: 4px;
    border-radius: 2px;
    background: var(--border);
    box-shadow: var(--shadow-inset);
  }

  .large .track {
    height: 6px;
    border-radius: 3px;
  }

  /* In-memory cache (#453): discreet, the progress goes over it. */
  .cached {
    position: absolute;
    inset: 0 auto 0 0;
    width: calc(var(--cached) * 100%);
    border-radius: inherit;
    background: color-mix(in oklab, var(--text) 22%, transparent);
    transition: width var(--motion-fast);
  }

  .fill {
    position: absolute;
    inset: 0 auto 0 0;
    width: calc(var(--ratio) * 100%);
    border-radius: inherit;
    background: var(--accent);
    box-shadow: 0 0 12px color-mix(in oklab, var(--accent) 60%, transparent);
  }

  input {
    position: absolute;
    inset: -8px 0;
    width: 100%;
    margin: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }

  input::-webkit-slider-thumb {
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text);
    opacity: 0;
    transition: opacity var(--motion-fast);
  }

  .progress:hover input::-webkit-slider-thumb,
  input:focus-visible::-webkit-slider-thumb {
    opacity: 1;
  }

  input:disabled {
    cursor: default;
  }

  :global([data-theme="studio"]) .track {
    border-radius: 0;
    background: repeating-linear-gradient(90deg, var(--border) 0 2px, transparent 2px 4px);
  }

  :global([data-theme="studio"]) .cached {
    border-radius: 0;
    background: repeating-linear-gradient(
      90deg,
      color-mix(in oklab, var(--text) 35%, transparent) 0 2px,
      transparent 2px 4px
    );
  }

  :global([data-theme="studio"]) .fill {
    border-radius: 0;
    box-shadow: none;
    background: repeating-linear-gradient(90deg, var(--accent) 0 2px, transparent 2px 4px);
  }

  :global([data-theme="atelier"]) .track {
    height: 2px;
  }

  :global([data-theme="atelier"]) .fill {
    box-shadow: none;
    background: var(--text);
  }
</style>
