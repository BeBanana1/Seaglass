<script lang="ts">
  // Grand hotel backdrop (#339): pink striped wallpaper, burgundy wainscoting at the bottom,
  // mustard frieze, pale blue medallion top center, all symmetrical. Nothing is animated.
  let { intense = false }: { intense?: boolean } = $props();
</script>

<div class="scene" class:intense aria-hidden="true">
  <div class="stripes"></div>
  <div class="wainscot"></div>
  <svg class="crest" viewBox="0 0 200 120">
    <ellipse cx="100" cy="60" rx="90" ry="54" fill="var(--gh-blue)" opacity="0.55" />
    <ellipse
      cx="100"
      cy="60"
      rx="80"
      ry="45"
      fill="none"
      stroke="var(--gh-burgundy)"
      stroke-width="1.5"
      opacity="0.5"
    />
    <ellipse
      cx="100"
      cy="60"
      rx="74"
      ry="40"
      fill="none"
      stroke="var(--gh-mustard)"
      stroke-width="3"
      opacity="0.7"
    />
    <path d="M100 32 L108 56 L100 88 L92 56 Z" fill="var(--gh-burgundy)" opacity="0.4" />
  </svg>
</div>

<style>
  .scene {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    background: var(--bg);
  }

  /* Wallpaper stripes: two pinks and a hairline, repeated identically. */
  .stripes {
    position: absolute;
    inset: 0;
    background: repeating-linear-gradient(
      90deg,
      transparent 0 34px,
      rgb(231 185 191 / 0.55) 34px 58px,
      rgb(78 20 36 / 0.1) 58px 59px,
      transparent 59px 60px
    );
    background-position: center;
  }

  /* Wainscoting: burgundy band, mustard frieze above it. */
  .wainscot {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    height: 12%;
    background:
      linear-gradient(var(--gh-mustard), var(--gh-mustard)) top / 100% 6px no-repeat,
      linear-gradient(rgb(251 241 239 / 0.7), rgb(251 241 239 / 0.7)) 0 10px / 100% 2px no-repeat,
      repeating-linear-gradient(90deg, #6a1d31 0 70px, #5a1829 70px 72px);
    opacity: 0.85;
  }

  .crest {
    position: absolute;
    top: 2%;
    left: 50%;
    width: 220px;
    transform: translateX(-50%);
  }

  /* "Now playing": paler wallpaper, medallion removed (it fell under the buttons). */
  .intense .stripes {
    opacity: 0.6;
  }

  .intense .crest {
    display: none;
  }
</style>
