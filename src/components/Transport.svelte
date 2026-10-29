<script lang="ts">
  import { commands, type Repeat } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import Icon from "./Icon.svelte";

  let { large = false }: { large?: boolean } = $props();

  const nextRepeat: Record<Repeat, Repeat> = { off: "all", all: "one", one: "off" };
  const repeatLabel: Record<Repeat, string> = {
    off: "Répéter : désactivé",
    all: "Répéter : tout",
    one: "Répéter : ce titre",
  };

  const s = $derived(player.state);
  const playing = $derived(!s.paused && !s.idle);
  const size = $derived(large ? 24 : 18);
</script>

<div class="transport" class:large>
  <button
    class="toggle"
    class:on={s.shuffle}
    onclick={() => commands.setShuffle(!s.shuffle)}
    title={s.shuffle ? "Lecture aléatoire : activée" : "Lecture aléatoire : désactivée"}
  >
    <Icon name="shuffle" size={size - 2} />
  </button>
  <button onclick={() => commands.previous()} disabled={!s.track} title="Précédent">
    <Icon name="previous" {size} />
  </button>
  <button
    class="play"
    onclick={() => commands.togglePause()}
    disabled={!s.track}
    aria-label={playing ? "Pause" : "Lecture"}
    title={playing ? "Pause (Espace)" : "Lecture (Espace)"}
  >
    {#if s.loading || s.buffering}
      <span class="spinner"></span>
    {:else}
      <Icon name={playing ? "pause" : "play"} {size} />
    {/if}
  </button>
  <button onclick={() => commands.next()} disabled={!s.hasNext} title="Suivant">
    <Icon name="next" {size} />
  </button>
  <button
    class="toggle"
    class:on={s.repeat !== "off"}
    onclick={() => commands.setRepeat(nextRepeat[s.repeat])}
    title={repeatLabel[s.repeat]}
  >
    <Icon name="repeat" size={size - 2} />
    {#if s.repeat === "one"}<span class="one">1</span>{/if}
  </button>
</div>

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .transport.large {
    gap: 28px;
  }

  button {
    position: relative;
    display: grid;
    place-items: center;
    padding: 4px;
    color: var(--text-muted);
    transition:
      color 0.15s,
      transform 0.15s;
  }

  button:hover:not(:disabled) {
    color: var(--text);
  }

  button:active:not(:disabled) {
    transform: scale(0.92);
  }

  button:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .toggle.on,
  .toggle.on:hover {
    color: var(--accent);
  }

  .toggle.on::after {
    content: "";
    position: absolute;
    bottom: -4px;
    width: 4px;
    height: 4px;
    border-radius: 50%;
    background: currentColor;
  }

  .one {
    position: absolute;
    top: -3px;
    right: -5px;
    font-size: 9px;
    font-weight: 800;
  }

  .play {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    color: var(--accent-contrast);
    background: var(--text);
  }

  .play:hover:not(:disabled) {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .large .play {
    width: 64px;
    height: 64px;
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

  :global([data-theme="studio"]) .play {
    border-radius: 0;
    background: var(--accent);
  }

  :global([data-theme="studio"]) .play:hover:not(:disabled) {
    background: var(--accent-strong);
  }

  :global([data-theme="atelier"]) button {
    color: var(--text);
  }

  :global([data-theme="atelier"]) .toggle.on {
    color: var(--accent);
  }

  :global([data-theme="atelier"]) .play {
    color: var(--surface);
    background: var(--text);
  }
</style>
