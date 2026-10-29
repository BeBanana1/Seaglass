<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type Repeat } from "../lib/ipc";
  import { estimatedPosition, player } from "../lib/stores/player.svelte";
  import { artistNames, formatStream, formatTime, fullTitle } from "../lib/format";
  import Icon from "./Icon.svelte";

  let { queueOpen, onToggleQueue }: { queueOpen: boolean; onToggleQueue: () => void } = $props();

  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
  const repeatLabel: Record<Repeat, string> = {
    off: "Répéter : désactivé",
    all: "Répéter : tout",
    one: "Répéter : ce titre",
  };

  let position = $state(0);
  /** Position en cours de glissement : on n'écrase pas la barre tant que l'utilisateur la tient. */
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
  const playing = $derived(!s.paused && !s.idle);

  async function seekTo(value: number) {
    dragging = null;
    await commands.seek(value);
  }

  function outputLabel(): string | null {
    if (!s.output) return null;
    const rate = (s.output.samplerate / 1000).toLocaleString("fr-FR");
    return `${s.outputDriver ?? "?"} · ${rate} kHz · ${s.output.format}`;
  }
</script>

<footer>
  <div class="now">
    {#if s.track}
      {#if s.track.album.coverUrl}
        <img src={s.track.album.coverUrl} alt="" />
      {/if}
      <div class="meta">
        <span class="title">{fullTitle(s.track)}</span>
        <span class="sub">{artistNames(s.track)}</span>
      </div>
    {:else}
      <span class="sub">Rien en lecture</span>
    {/if}
  </div>

  <div class="center">
    <div class="transport">
      <button
        class="toggle"
        class:on={s.shuffle}
        onclick={() => commands.setShuffle(!s.shuffle)}
        title={s.shuffle ? "Lecture aléatoire : activée" : "Lecture aléatoire : désactivée"}
      >
        <Icon name="shuffle" />
      </button>
      <button onclick={() => commands.previous()} disabled={!s.track} title="Précédent">
        <Icon name="previous" />
      </button>
      <button
        class="play"
        onclick={() => commands.togglePause()}
        disabled={!s.track}
        aria-label={playing ? "Pause" : "Lecture"}
      >
        {#if s.loading || s.buffering}
          <span class="spinner"></span>
        {:else}
          <Icon name={playing ? "pause" : "play"} />
        {/if}
      </button>
      <button onclick={() => commands.next()} disabled={!s.hasNext} title="Suivant">
        <Icon name="next" />
      </button>
      <button
        class="toggle"
        class:on={s.repeat !== "off"}
        onclick={() => commands.setRepeat(nextRepeat[s.repeat])}
        title={repeatLabel[s.repeat]}
      >
        <Icon name="repeat" />
        {#if s.repeat === "one"}<span class="one">1</span>{/if}
      </button>
    </div>
    <div class="progress">
      <span class="time">{formatTime(shown)}</span>
      <input
        type="range"
        min="0"
        max={s.duration || 1}
        step="0.1"
        value={shown}
        disabled={!s.track || !s.duration}
        oninput={(e) => (dragging = e.currentTarget.valueAsNumber)}
        onchange={(e) => seekTo(e.currentTarget.valueAsNumber)}
        style:--progress="{s.duration ? (shown / s.duration) * 100 : 0}%"
      />
      <span class="time">{formatTime(s.duration)}</span>
    </div>
  </div>

  <div class="right">
    <div class="format">
      {#if s.stream}
        <span class="badge" class:muted={s.stream.codec !== "flac"}>{formatStream(s.stream)}</span>
      {/if}
      {#if outputLabel()}
        <span class="sub" title="Format envoyé à la sortie audio">{outputLabel()}</span>
      {/if}
    </div>
    <button class="toggle" class:on={queueOpen} onclick={onToggleQueue} title="File d'attente">
      <Icon name="queue" />
    </button>
  </div>
</footer>

<style>
  footer {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 2fr) minmax(0, 1fr);
    align-items: center;
    gap: 16px;
    padding: 0 20px;
    border-top: 1px solid var(--border);
    background: var(--bg-raised);
  }

  .now {
    display: flex;
    align-items: center;
    gap: 12px;
    min-width: 0;
  }

  img {
    width: 52px;
    height: 52px;
    border-radius: 4px;
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title,
  .sub {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .sub {
    color: var(--text-muted);
    font-size: 12px;
  }

  .center {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
  }

  .play {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    color: var(--bg);
    background: var(--text);
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .transport button:not(.play),
  .right .toggle {
    position: relative;
    padding: 4px;
    color: var(--text-muted);
  }

  .transport button:not(.play):hover:not(:disabled),
  .right .toggle:hover {
    color: var(--text);
  }

  .toggle.on,
  .toggle.on:hover {
    color: var(--accent) !important;
  }

  .one {
    position: absolute;
    top: -2px;
    right: -4px;
    font-size: 9px;
    font-weight: 700;
  }

  button:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .progress {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .time {
    min-width: 40px;
    font-size: 11px;
    font-variant-numeric: tabular-nums;
    color: var(--text-muted);
    text-align: center;
  }

  input[type="range"] {
    flex: 1;
    height: 4px;
    appearance: none;
    border-radius: 2px;
    background: linear-gradient(
      to right,
      var(--accent) var(--progress),
      var(--border) var(--progress)
    );
    cursor: pointer;
  }

  input[type="range"]::-webkit-slider-thumb {
    appearance: none;
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--text);
  }

  input[type="range"]:disabled {
    cursor: default;
  }

  .right {
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    min-width: 0;
  }

  .format {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 4px;
    min-width: 0;
  }
</style>
