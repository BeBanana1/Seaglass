<script lang="ts">
  // "Featured" banner of the home page: a highlighted album, tinted with its own color.
  import type { AlbumSummary } from "../lib/ipc";
  import { albumKind, coverLarge, year } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let {
    album,
    context,
    onskip,
  }: {
    album: AlbumSummary;
    context: string;
    /** Skip to another album (#185); absent if there is no other. */
    onskip?: () => void;
  } = $props();

  const large = $derived(coverLarge(album));
</script>

<!-- The Skip button sits next to the banner, not inside it: a button cannot contain another. -->
<div class="hero-wrap">
  <button
    class="hero"
    style:--tint={album.color ?? "var(--album)"}
    onclick={() => go({ name: "album", id: album.id })}
  >
    {#if large}<img class="bg" src={large} alt="" aria-hidden="true" />{/if}
    <div class="content">
      <span class="label">{t.views.home.featured(context)}</span>
      <span class="title display">{album.title}</span>
      <span class="meta">
        {[album.artists.map((a) => a.name).join(", "), albumKind(album), year(album.releaseDate)]
          .filter(Boolean)
          .join(" · ")}
        {#if album.hires}<span class="badge">HI-RES</span>{/if}
      </span>
    </div>
    {#if large}<img class="cover" src={large} alt="" />{/if}
  </button>
  {#if onskip}
    <button class="hero-skip" onclick={onskip} title={t.views.home.skipFeaturedHint}>
      <Icon name="next" size={14} />
      <span>{t.views.home.skipFeatured}</span>
    </button>
  {/if}
</div>

<style>
  .hero-wrap {
    position: relative;
  }

  /* Over the banner, whatever the theme: light text on a dark veil. Visible on
     hover or focus on desktop, always on Android (no hover). */
  .hero-skip {
    position: absolute;
    top: 10px;
    right: 20px;
    z-index: 2;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 12px;
    border-radius: var(--radius-pill);
    font-size: 12px;
    font-weight: 600;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    color: #fff;
    background: rgb(0 0 0 / 0.5);
    opacity: 0;
    transition: opacity var(--motion-fast, 0.15s);
  }

  .hero-wrap:hover .hero-skip,
  .hero-skip:focus-visible {
    opacity: 1;
  }

  .hero-skip:hover {
    background: rgb(0 0 0 / 0.7);
  }

  :global(:root[data-platform="android"]) .hero-skip {
    top: 6px;
    right: 16px;
    padding: 8px;
    opacity: 1;
  }

  :global(:root[data-platform="android"]) .hero-skip span {
    display: none;
  }

  .hero {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    width: calc(100% - 20px);
    min-height: 240px;
    margin: 0 10px 32px;
    padding: 28px 32px;
    border-radius: calc(var(--radius) + 6px);
    overflow: hidden;
    text-align: left;
    color: #fff;
    background: color-mix(in oklab, var(--tint) 70%, #000);
    isolation: isolate;
  }

  .bg {
    position: absolute;
    inset: -30%;
    z-index: -1;
    width: 160%;
    height: 160%;
    object-fit: cover;
    filter: blur(60px) saturate(1.5) brightness(0.7);
    transition: transform 6s ease-out;
  }

  .hero:hover .bg {
    transform: scale(1.1);
  }

  .content {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 60%;
  }

  .label {
    color: rgb(255 255 255 / 0.75);
  }

  .title {
    font-size: clamp(26px, 3.4vw, 40px);
    line-height: 1.1;
  }

  .meta {
    color: rgb(255 255 255 / 0.8);
  }

  .cover {
    width: 184px;
    height: 184px;
    flex-shrink: 0;
    border-radius: var(--radius-cover, var(--radius));
    box-shadow: var(--shadow-cover, 0 20px 50px rgb(0 0 0 / 0.45));
    transform: rotate(3deg);
    transition: transform 0.4s;
  }

  .hero:hover .cover {
    transform: rotate(0deg) scale(1.03);
  }

  :global([data-theme="studio"]) .hero {
    border-radius: 0;
    border: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .cover {
    transform: none;
  }

  /* Atelier: magazine cover, no blur, flat color. */
  :global([data-theme="atelier"]) .hero {
    border-radius: 0;
    color: var(--text);
    background: color-mix(in oklab, var(--tint) 22%, var(--bg));
  }

  :global([data-theme="atelier"]) .bg {
    display: none;
  }

  :global([data-theme="atelier"]) .label,
  :global([data-theme="atelier"]) .meta {
    color: var(--text-muted);
  }

  :global([data-theme="atelier"]) .cover {
    box-shadow: 12px 12px 0 color-mix(in oklab, var(--tint) 45%, var(--bg));
    transform: none;
  }

  /* Android (#24, #184): low banner, cover to the right of the text, to also see the first
     row of the home page in full. Title limited to two lines. */
  :global(:root[data-platform="android"]) .hero {
    gap: 14px;
    min-height: 0;
    margin-bottom: 24px;
    padding: 16px;
  }

  :global(:root[data-platform="android"]) .content {
    flex: 1;
    min-width: 0;
    max-width: none;
    gap: 6px;
  }

  :global(:root[data-platform="android"]) .title {
    display: -webkit-box;
    overflow: hidden;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    font-size: 21px;
  }

  :global(:root[data-platform="android"]) .cover {
    width: 96px;
    height: 96px;
    transform: none;
  }
</style>
