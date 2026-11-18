<script lang="ts">
  // Clins d'œil (#399) : filtre SVG des pochettes en gros pixels (séquence au clavier), posé
  // sur les images seulement pendant l'effet. Les images elles-mêmes ne changent pas.
</script>

<svg class="egg-defs" aria-hidden="true" width="0" height="0">
  <filter id="egg-pixelate" x="0" y="0" width="1" height="1">
    <!-- Un point par case de 10 px, étalé ensuite sur toute la case. -->
    <feFlood x="5" y="5" width="1" height="1" />
    <feComposite width="10" height="10" />
    <feTile result="grid" />
    <feComposite in="SourceGraphic" in2="grid" operator="in" />
    <feMorphology operator="dilate" radius="5" />
  </filter>
</svg>

<style>
  .egg-defs {
    position: absolute;
    width: 0;
    height: 0;
    overflow: hidden;
  }

  :global(:root[data-egg-pixels] img) {
    filter: url(#egg-pixelate);
  }
</style>
