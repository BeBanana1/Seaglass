<script lang="ts">
  // Easter eggs (#399): SVG filter turning covers into big pixels (keyboard sequence), applied
  // to images only during the effect. The images themselves do not change.
</script>

<svg class="egg-defs" aria-hidden="true" width="0" height="0">
  <filter id="egg-pixelate" x="0" y="0" width="1" height="1">
    <!-- One dot per 10 px cell, then spread over the whole cell. -->
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
