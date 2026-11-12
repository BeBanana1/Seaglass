<script lang="ts">
  // Fond Séance (#339) : la salle obscure. Rideaux de velours rouge sur les bords (plis en
  // dégradés), faisceau du projecteur qui descend du haut, rangées de fauteuils en silhouette
  // en bas, grain de pellicule fixe (`feTurbulence`). Rien n'est animé.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <div class="beam"></div>
  <div class="curtain left"></div>
  <div class="curtain right"></div>
  <svg class="seats" viewBox="0 0 1600 120" preserveAspectRatio="none">
    <path
      d="M0 120 V70 Q40 40 80 70 Q120 40 160 70 Q200 40 240 70 Q280 40 320 70 Q360 40 400 70 Q440 40 480 70 Q520 40 560 70 Q600 40 640 70 Q680 40 720 70 Q760 40 800 70 Q840 40 880 70 Q920 40 960 70 Q1000 40 1040 70 Q1080 40 1120 70 Q1160 40 1200 70 Q1240 40 1280 70 Q1320 40 1360 70 Q1400 40 1440 70 Q1480 40 1520 70 Q1560 40 1600 70 V120 Z"
    />
  </svg>
  <svg class="grain" width="100%" height="100%">
    <filter id="se-grain">
      <feTurbulence type="fractalNoise" baseFrequency="0.9" numOctaves="2" seed="7" stitchTiles="stitch" />
      <feColorMatrix values="0 0 0 0 0.94  0 0 0 0 0.9  0 0 0 0 0.82  0 0 0 0.55 -0.12" />
    </filter>
    <rect width="100%" height="100%" filter="url(#se-grain)" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: radial-gradient(ellipse 80% 60% at 50% 40%, #1a1214, var(--bg) 70%);
  }

  /* Faisceau : cône pâle depuis la cabine, au-dessus du centre. */
  .beam {
    position: absolute;
    inset: 0;
    background: conic-gradient(
      from 168deg at 50% -20%,
      transparent 0deg,
      rgb(246 239 224 / 0.05) 8deg,
      rgb(246 239 224 / 0.08) 12deg,
      rgb(246 239 224 / 0.05) 16deg,
      transparent 24deg
    );
  }

  /* Rideaux : plis verticaux du velours, plus sombres vers la salle. */
  .curtain {
    position: absolute;
    top: 0;
    bottom: 0;
    width: clamp(48px, 7vw, 140px);
    background:
      linear-gradient(90deg, transparent, rgb(0 0 0 / 0.55)),
      repeating-linear-gradient(
        90deg,
        var(--se-velvet-deep) 0 6px,
        var(--se-velvet) 14px,
        #9a2029 18px,
        var(--se-velvet) 22px,
        var(--se-velvet-deep) 32px
      );
    opacity: 0.85;
  }

  .curtain.left {
    left: 0;
    -webkit-mask: linear-gradient(90deg, black 55%, transparent);
    mask: linear-gradient(90deg, black 55%, transparent);
  }

  .curtain.right {
    right: 0;
    transform: scaleX(-1);
    -webkit-mask: linear-gradient(90deg, black 55%, transparent);
    mask: linear-gradient(90deg, black 55%, transparent);
  }

  .seats {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    width: 100%;
    height: 70px;
    fill: #050304;
    opacity: 0.9;
  }

  .grain {
    position: absolute;
    inset: 0;
    opacity: 0.06;
    mix-blend-mode: screen;
  }

  /* « En lecture » : les rideaux reculent, la salle devient plus noire. */
  .intense .curtain {
    width: clamp(32px, 4vw, 80px);
    opacity: 0.6;
  }

  .intense .beam {
    opacity: 0.6;
  }
</style>
