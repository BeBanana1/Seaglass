<script lang="ts">
  // Fond Aéro (#101) : planche de livrée d'avion. Bande jaune verticale à droite, bande
  // diagonale jaune / ardoise en bas à gauche (pas dans « En lecture »), trame de points,
  // règle graduée en bas.
  // Seule la règle du bas défile, lentement, pendant la lecture (`data-playing` sur <html>).
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMid slice">
    <defs>
      <pattern id="aero-dots" width="16" height="16" patternUnits="userSpaceOnUse">
        <circle cx="8" cy="8" r="1.4" fill="#1f2f35" opacity="0.28" />
      </pattern>
      <pattern id="aero-ticks" width="40" height="24" patternUnits="userSpaceOnUse">
        <rect x="0" y="0" width="2" height="24" fill="#1f2f35" />
        <rect x="20" y="12" width="2" height="12" fill="#1f2f35" />
      </pattern>
    </defs>

    <!-- Bande jaune à droite, filet ardoise sur sa gauche. -->
    <rect x="1490" y="0" width="110" height="1000" fill="#f2ee1d" />
    <rect x="1486" y="0" width="4" height="1000" fill="#1f2f35" />

    <!-- Trame de points (milieu gauche) et bloc bleu plus foncé (bas droit). -->
    <rect x="40" y="180" width="300" height="360" fill="url(#aero-dots)" />
    <rect x="1180" y="700" width="306" height="300" fill="#bcdde2" />

    <!-- Bande diagonale de livrée : ardoise puis jaune, depuis le coin bas gauche. -->
    <g class="livery" transform="rotate(-38 0 1000)">
      <rect x="-200" y="900" width="900" height="54" fill="#1f2f35" />
      <rect x="-200" y="954" width="900" height="70" fill="#f2ee1d" />
      <rect x="-200" y="1024" width="900" height="6" fill="#1f2f35" />
    </g>

    <!-- Repères : tirets, points, petit carré encadré. -->
    <g fill="#1f2f35">
      <rect x="40" y="60" width="28" height="3" />
      <rect x="40" y="600" width="28" height="3" />
      <circle cx="1440" cy="60" r="3" />
      <circle cx="1440" cy="600" r="3" />
      <circle cx="54" cy="640" r="5" fill="#f2ee1d" stroke="#1f2f35" stroke-width="2" />
      <circle cx="54" cy="656" r="5" />
      <circle cx="54" cy="672" r="5" />
    </g>
    <rect x="1380" y="120" width="72" height="56" fill="none" stroke="#1f2f35" stroke-width="3" />
    <path d="M1396 160 L1416 132 L1436 160 Z" fill="#1f2f35" />

    <!-- Règle graduée en bas (défile pendant la lecture). -->
    <g class="ruler-wrap">
      <rect class="ruler" x="-40" y="976" width="1680" height="24" fill="url(#aero-ticks)" />
    </g>
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
  }

  svg {
    width: 100%;
    height: 100%;
  }

  .intense svg {
    opacity: 0.6;
  }

  /* Dans « En lecture », la bande passerait sous le titre et ses libellés. */
  .intense .livery {
    display: none;
  }

  .ruler {
    opacity: 0.45;
  }

  :global(:root[data-playing="true"]) .ruler {
    animation: slide 6s linear infinite;
  }

  @keyframes slide {
    to {
      transform: translateX(40px);
    }
  }
</style>
