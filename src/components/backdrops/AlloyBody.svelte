<script lang="ts">
  // Fond Alliage (#175) : un flanc de carrosserie en aluminium. Métal brossé (stries fines
  // horizontales), reflet large en diagonale, ligne de caisse (filet sombre + filet clair)
  // qui sépare un bas de caisse plus sombre, ouïes en biais en bas à gauche.
  // Rien d'animé : dégradés fixes, aucun flou (#146, #158). Les couleurs viennent des jetons
  // du thème (`--al-metal`, `--al-hi`, `--al-lo`), donc la même scène sert aux deux variantes.
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
      repeating-linear-gradient(0deg, color-mix(in srgb, var(--al-hi) 30%, transparent) 0 1px, transparent 1px 2px),
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

  /* Reflet large et doux, comme la lumière du ciel sur une portière. */
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

  /* Ligne de caisse : au-dessous, le bas de caisse plus sombre. */
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

  /* Ouïes en biais, en bas à gauche, sur le bas de caisse. */
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

  /* Dans « En lecture » et sur Android, les ouïes passeraient sous les commandes. */
  .intense .louvers,
  :global(:root[data-platform="android"]) .louvers {
    display: none;
  }
</style>
