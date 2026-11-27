<script lang="ts">
  // Surrealism backdrop: vector landscape drawn in SVG, tinted by the album color
  // (`--album`). Arch, staircase leading nowhere, sphere and cube float gently.
  // Objects stay on the edges: the center is taken by the content.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <svg viewBox="0 0 1600 1000" preserveAspectRatio="xMidYMax slice">
    <defs>
      <linearGradient id="sky" x1="0" y1="0" x2="0" y2="1">
        <stop offset="0" style="stop-color: var(--sky-top)" />
        <stop offset="0.7" style="stop-color: var(--sky-low)" />
      </linearGradient>
      <radialGradient id="sun" cx="0.4" cy="0.35" r="0.7">
        <stop offset="0" stop-color="#fff6e8" />
        <stop offset="1" style="stop-color: var(--sun)" />
      </radialGradient>
      <linearGradient id="sphere" x1="0" y1="0" x2="1" y2="1">
        <stop offset="0" stop-color="#fffaf2" />
        <stop offset="1" style="stop-color: var(--accent-soft)" />
      </linearGradient>
      <filter id="grain">
        <feTurbulence type="fractalNoise" baseFrequency="0.8" numOctaves="2" stitchTiles="stitch" />
        <feColorMatrix values="0 0 0 0 0.25  0 0 0 0 0.18  0 0 0 0 0.22  0 0 0 0.5 0" />
      </filter>
    </defs>

    <rect width="1600" height="1000" fill="url(#sky)" />

    <!-- Low sun, on the right, half set behind the hills. -->
    <circle class="sun" cx="1330" cy="560" r="170" fill="url(#sun)" />

    <!-- Pill-shaped clouds. -->
    <g class="cloud slow">
      <rect x="120" y="90" width="240" height="52" rx="26" fill="#fffaf3" opacity="0.85" />
      <rect x="185" y="60" width="140" height="52" rx="26" fill="#fffaf3" opacity="0.85" />
    </g>
    <g class="cloud">
      <rect x="980" y="70" width="180" height="42" rx="21" fill="#fffaf3" opacity="0.7" />
    </g>

    <!-- Hills. -->
    <path
      d="M0 640 C 260 560 460 600 720 650 S 1200 560 1600 620 V1000 H0Z"
      style="fill: var(--hill-1)"
    />
    <path
      d="M0 760 C 300 690 560 720 820 770 S 1300 700 1600 740 V1000 H0Z"
      style="fill: var(--hill-2)"
    />
    <path
      d="M0 860 C 400 820 700 850 1000 880 S 1400 840 1600 860 V1000 H0Z"
      style="fill: var(--hill-3)"
    />

    <!-- Arch resting on the right hill, with a shadow that is too long. -->
    <path d="M1420 742 L 1180 772 L 1196 782 L 1430 756 Z" fill="#3d2a4d" opacity="0.12" />
    <path
      d="M1420 742 V 560 A 56 56 0 0 1 1532 560 V 742 H 1506 V 566 A 30 30 0 0 0 1446 566 V 742 Z"
      fill="#fffaf2"
    />

    <!-- Staircase leading nowhere, hanging at the top right. -->
    <g class="float">
      <path d="M1300 190 h40 v-28 h40 v-28 h40 v-28 h40 v84 Z" style="fill: var(--accent-soft)" />
      <path d="M1300 190 h160 v10 h-160 Z" fill="#3d2a4d" opacity="0.16" />
    </g>

    <!-- Sphere levitating on the left, above its shadow. -->
    <g class="float slow">
      <circle cx="80" cy="520" r="46" fill="url(#sphere)" />
    </g>
    <ellipse class="shadow slow" cx="80" cy="690" rx="40" ry="7" fill="#3d2a4d" opacity="0.14" />

    <!-- Small tilted cube, at the top left. -->
    <g class="float fast">
      <rect
        x="70"
        y="250"
        width="40"
        height="40"
        rx="4"
        transform="rotate(18 90 270)"
        style="fill: var(--sun)"
      />
    </g>

    <rect width="1600" height="1000" filter="url(#grain)" opacity="0.16" />
  </svg>
</div>

<style>
  /* Mixes in oklch: the album tint is kept instead of turning grey. */
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    --sky-top: color-mix(in oklch, var(--album) 38%, #fbeee6);
    --sky-low: color-mix(in oklch, var(--album) 12%, #fde3d2);
    --sun: color-mix(in oklch, var(--album) 30%, #f7a985);
    --hill-1: color-mix(in oklch, var(--album) 34%, #efc9c0);
    --hill-2: color-mix(in oklch, var(--album) 24%, #e9b7ab);
    --hill-3: color-mix(in oklch, var(--album) 16%, #f6d8c8);
    --accent-soft: color-mix(in oklch, var(--album) 60%, #fffaf2);
  }

  svg {
    width: 100%;
    height: 100%;
  }

  .float {
    animation: float 9s ease-in-out infinite alternate;
  }

  .shadow {
    transform-box: fill-box;
    transform-origin: center;
    animation: shadow 9s ease-in-out infinite alternate;
  }

  .slow {
    animation-duration: 13s;
  }

  .fast {
    animation-duration: 6s;
  }

  .cloud {
    animation: cloud 60s linear infinite alternate;
  }

  .cloud.slow {
    animation-duration: 90s;
  }

  .sun {
    animation: sun 30s ease-in-out infinite alternate;
  }

  @keyframes float {
    to {
      transform: translateY(-28px);
    }
  }

  @keyframes shadow {
    to {
      transform: scaleX(0.8);
      opacity: 0.08;
    }
  }

  @keyframes cloud {
    to {
      transform: translateX(120px);
    }
  }

  @keyframes sun {
    to {
      transform: translateY(-24px);
    }
  }
</style>
