<script lang="ts">
  import { onMount } from "svelte";
  import { commands } from "../lib/ipc";
  import { estimatedPosition, player } from "../lib/stores/player.svelte";
  import { formatTime } from "../lib/format";

  let { large = false }: { large?: boolean } = $props();

  let position = $state(0);
  /** Valeur en cours de glissement : la barre ne suit plus la lecture tant qu'on la tient. */
  let dragging = $state<number | null>(null);

  onMount(() => {
    let frame = requestAnimationFrame(function tick(now) {
      position = estimatedPosition(now);
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  });

  const s = $derived(player.state);
  const shown = $derived(dragging ?? position);
  const ratio = $derived(s.duration ? Math.min(shown / s.duration, 1) : 0);

  async function seekTo(value: number) {
    dragging = null;
    await commands.seek(value);
  }
</script>

<div class="progress" class:large>
  <span class="time mono">{formatTime(shown)}</span>
  <div class="track" style:--ratio={ratio}>
    <div class="fill"></div>
    <input
      type="range"
      min="0"
      max={s.duration || 1}
      step="0.1"
      value={shown}
      disabled={!s.track || !s.duration}
      aria-label="Position"
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
  }

  .large .track {
    height: 6px;
    border-radius: 3px;
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
    transition: opacity 0.15s;
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
