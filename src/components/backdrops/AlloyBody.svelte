<script lang="ts">
  // Alloy backdrop (#175): the side panel of an aluminium car body. Brushed metal (fine
  // horizontal grain), wide diagonal highlight, character line (dark hairline + light one)
  // separating a darker rocker panel, slanted vents at the bottom left.
  // Nothing animated: fixed gradients, no blur (#146, #158). Colors come from the theme
  // tokens (`--al-metal`, `--al-hi`, `--al-lo`), so the same scene serves both variants.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="body" class:intense aria-hidden="true">
  <div class="sheen"></div>
  <div class="crease"></div>
  <div class="louvers"></div>
</div>

<style>
  .body {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background:
      repeating-linear-gradient(
        0deg,
        color-mix(in srgb, var(--al-hi) 30%, transparent) 0 1px,
        transparent 1px 2px
      ),
      linear-gradient(
        172deg,
        color-mix(in srgb, var(--al-metal) 80%, white) 0%,
        var(--al-metal) 34%,
        color-mix(in srgb, var(--al-metal) 88%, white) 46%,
        var(--al-metal) 58%,
        color-mix(in srgb, var(--al-metal) 86%, black) 100%
      );
    background-blend-mode: soft-light, normal;
  }

  /* Wide, soft highlight, like sky light on a door. */
  .sheen {
    position: absolute;
    inset: -20% -10%;
    background: linear-gradient(
      100deg,
      transparent 30%,
      color-mix(in srgb, white 22%, transparent) 44%,
      transparent 56%
    );
  }

  /* Character line: below it, the darker rocker panel. */
  .crease {
    position: absolute;
    left: -10%;
    right: -10%;
    top: 68%;
    bottom: -20%;
    transform: rotate(-4deg);
    transform-origin: 0 0;
    background: color-mix(in srgb, var(--al-metal) 90%, black);
    box-shadow:
      0 -1px 0 var(--al-lo),
      0 1px 0 var(--al-hi) inset;
  }

  /* Slanted vents, at the bottom left, on the rocker panel. */
  .louvers {
    position: absolute;
    left: 3%;
    bottom: 5%;
    width: 220px;
    height: 34px;
    background: repeating-linear-gradient(-55deg, var(--al-lo) 0 3px, transparent 3px 10px);
    clip-path: polygon(8px 0, 100% 0, 100% 100%, 0 100%, 0 8px);
  }

  .intense .crease {
    opacity: 0.6;
  }

  /* In "Now playing" and on Android, the vents would pass under the controls. */
  .intense .louvers,
  :global(:root[data-platform="android"]) .louvers {
    display: none;
  }
</style>
