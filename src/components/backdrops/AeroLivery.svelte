<script lang="ts">
  // Aero backdrop (#101): aircraft livery sheet. Vertical yellow band on the right,
  // yellow / slate diagonal band at the bottom left (not in "Now playing"), dot grid,
  // graduated ruler at the bottom.
  // Only the bottom ruler scrolls, slowly, during playback (`data-playing` on <html>).
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

    <!-- Yellow band on the right, slate hairline on its left. -->
    <rect x="1490" y="0" width="110" height="1000" fill="#f2ee1d" />
    <rect x="1486" y="0" width="4" height="1000" fill="#1f2f35" />

    <!-- Dot grid (middle left) and darker blue block (bottom right). -->
    <rect x="40" y="180" width="300" height="360" fill="url(#aero-dots)" />
    <rect x="1180" y="700" width="306" height="300" fill="#bcdde2" />

    <!-- Diagonal livery band: slate then yellow, from the bottom left corner. -->
    <g class="livery" transform="rotate(-38 0 1000)">
      <rect x="-200" y="900" width="900" height="54" fill="#1f2f35" />
      <rect x="-200" y="954" width="900" height="70" fill="#f2ee1d" />
      <rect x="-200" y="1024" width="900" height="6" fill="#1f2f35" />
    </g>

    <!-- Marks: dashes, dots, small framed square. -->
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

    <!-- Graduated ruler at the bottom (scrolls during playback). -->
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

  /* In "Now playing", the band would pass under the title and its labels. */
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
