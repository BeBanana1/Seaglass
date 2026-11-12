<script lang="ts">
  // Clin d'œil (#399), thème Séance : quand la file s'arrête d'elle-même, un carton « Fin »
  // s'affiche quelques secondes, comme à la dernière image d'un film. Il ne bloque rien
  // (aucun clic capté) ; animations réduites : il apparaît et disparaît sans fondu.
  import { player } from "../lib/stores/player.svelte";
  import { ui } from "../lib/stores/ui.svelte";
  import { t } from "../lib/i18n.svelte";

  const SHOWN_MS = 4000;

  let shown = $state(false);

  $effect(() => {
    if (!player.ended || !ui.settings.easterEggs || ui.settings.theme !== "seance") return;
    shown = true;
    const timer = setTimeout(() => (shown = false), SHOWN_MS);
    return () => {
      clearTimeout(timer);
      shown = false;
    };
  });
</script>

{#if shown}
  <div class="end-card" aria-hidden="true">
    <span class="display">{t.eggs.theEnd}</span>
  </div>
{/if}

<style>
  .end-card {
    position: fixed;
    inset: 0;
    z-index: 58;
    display: grid;
    place-items: center;
    pointer-events: none;
    background: radial-gradient(ellipse at center, rgb(0 0 0 / 0.55), rgb(0 0 0 / 0.85));
    animation: end-card 4s ease-in-out 1 forwards;
  }

  span {
    padding: 18px 48px;
    border: 1px solid color-mix(in oklab, #f2e8d0 60%, transparent);
    color: #f2e8d0;
    font-size: 64px;
    font-style: italic;
    letter-spacing: 0.08em;
  }

  @keyframes end-card {
    0%,
    100% {
      opacity: 0;
    }
    15%,
    80% {
      opacity: 1;
    }
  }

  :global(:root[data-motion="reduced"]) .end-card {
    animation: none;
  }
</style>
