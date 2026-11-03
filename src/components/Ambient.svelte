<script lang="ts">
  // Fond d'ambiance : la pochette en cours, très floutée, en fondu entre deux titres.
  import { player } from "../lib/stores/player.svelte";

  let { intensity = 1 }: { intensity?: number } = $props();

  const cover = $derived(player.currentTrack?.album.coverUrl ?? null);
</script>

<div class="ambient" aria-hidden="true" style:--intensity={intensity}>
  {#key cover}
    {#if cover}<img src={cover} alt="" />{/if}
  {/key}
  <div class="veil"></div>
</div>

<style>
  .ambient {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background:
      radial-gradient(circle at 20% 10%, color-mix(in oklab, var(--album) 45%, transparent), transparent 60%),
      radial-gradient(circle at 90% 90%, color-mix(in oklab, var(--album) 30%, transparent), transparent 55%),
      var(--bg);
  }

  img {
    position: absolute;
    inset: -20%;
    width: 140%;
    height: 140%;
    object-fit: cover;
    filter: blur(90px) saturate(1.6);
    opacity: calc(0.55 * var(--intensity));
    animation:
      fade 1.2s ease-out,
      drift 40s ease-in-out infinite alternate;
  }

  /* Android (#146) : pas de dérive. Ce calque flouté à 90 px, animé sans fin (même en
     pause), faisait recomposer l'écran à chaque image ; le fondu entre deux pochettes reste. */
  :global(:root[data-platform="android"]) img {
    animation: fade 1.2s ease-out;
  }

  .veil {
    position: absolute;
    inset: 0;
    background: linear-gradient(180deg, rgb(6 16 18 / 0.35), rgb(6 16 18 / 0.75));
  }

  @keyframes fade {
    from {
      opacity: 0;
    }
  }

  @keyframes drift {
    to {
      transform: scale(1.12) rotate(4deg);
    }
  }
</style>
