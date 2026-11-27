<script lang="ts">
  // Acidwave backdrop: holographic washes, perspective grid, halftone and grain.
  // Animations only run during playback (`data-playing` on <html>).
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="field" class:intense aria-hidden="true">
  <div class="blob a"></div>
  <div class="blob b"></div>
  <div class="blob c"></div>
  <div class="grid"></div>
  <div class="scan"></div>
  <div class="grain"></div>
</div>

<style>
  .field {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: #050507;
  }

  .blob {
    position: absolute;
    width: 70vmax;
    height: 70vmax;
    border-radius: 50%;
    filter: blur(80px);
    opacity: 0.34;
    mix-blend-mode: screen;
    animation: drift 18s ease-in-out infinite alternate;
  }

  .intense .blob {
    opacity: 0.55;
  }

  .a {
    top: -30vmax;
    left: -20vmax;
    background: radial-gradient(circle, var(--accent), transparent 65%);
  }

  .b {
    right: -25vmax;
    top: 10vmax;
    background: radial-gradient(circle, var(--acid-magenta), transparent 65%);
    animation-duration: 23s;
    animation-direction: alternate-reverse;
  }

  .c {
    bottom: -40vmax;
    left: 20vmax;
    background: radial-gradient(
      circle,
      color-mix(in oklab, var(--album) 50%, var(--acid-uv)),
      transparent 65%
    );
    animation-duration: 29s;
  }

  /* Perspective grid floor, scrolling during playback. */
  .grid {
    position: absolute;
    left: -50%;
    right: -50%;
    bottom: -10%;
    height: 55%;
    background-image:
      linear-gradient(color-mix(in oklab, var(--accent) 45%, transparent) 1px, transparent 1px),
      linear-gradient(
        90deg,
        color-mix(in oklab, var(--acid-magenta) 40%, transparent) 1px,
        transparent 1px
      );
    background-size: 64px 64px;
    transform: perspective(380px) rotateX(62deg);
    transform-origin: bottom;
    mask-image: linear-gradient(to top, black 10%, transparent 85%);
    opacity: 0.35;
    animation: scroll 2.4s linear infinite;
  }

  .scan {
    position: absolute;
    inset: 0;
    background: repeating-linear-gradient(0deg, rgb(0 0 0 / 0.22) 0 1px, transparent 1px 3px);
  }

  .grain {
    position: absolute;
    inset: -50%;
    opacity: 0.08;
    background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='160'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='2' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
  }

  .blob,
  .grid {
    animation-play-state: paused;
  }

  :global(:root[data-playing="true"]) .blob,
  :global(:root[data-playing="true"]) .grid {
    animation-play-state: running;
  }

  @keyframes drift {
    to {
      transform: translate(12vmax, 8vmax) scale(1.2);
    }
  }

  @keyframes scroll {
    to {
      background-position: 0 64px;
    }
  }
</style>
