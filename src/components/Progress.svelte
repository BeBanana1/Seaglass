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
  /** Valeur en cours de glissement : la barre ne suit plus la lecture tant qu'on la tient. */
  let dragging = $state<number | null>(null);

  // Boucle d'images seulement pendant la lecture (#88).
  $effect(() => followPosition((p) => (position = p)));

  const s = $derived(player.state);
  const duration = $derived(s.duration);
  const shown = $derived(dragging ?? position);
  /** 1er août (#399) : la barre suit la trotteuse d'une horloge de gare, arrêt compris.
   *  Pas pendant un glissement, ni dans le temps affiché en chiffres. */
  const bar = $derived(dragging === null && eggs.date === "swissDay" ? stationClock(position, duration) : shown);
  const ratio = $derived(duration ? Math.min(bar / duration, 1) : 0);
  /** Partie déjà en mémoire (#453), en bande plus claire sous la progression. */
  const cachedRatio = $derived(s.track && duration ? Math.min(s.cached / duration, 1) : 0);
  /** Le texte du temps ne change qu'à la seconde : pas de recalcul à chaque image. */
  const shownText = $derived(formatTime(Math.floor(shown)));
  /** Dernières secondes du titre, quand le suivant est préchargé (même seuil de 30 s que
   *  l'acteur) : simple repère que les thèmes peuvent montrer (Séance, #339). */
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

  /* Cache en mémoire (#453) : discret, la progression passe par-dessus. */
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
