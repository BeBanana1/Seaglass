<script lang="ts">
  // Bandeau « À la une » de l'accueil : un album mis en avant, teinté de sa propre couleur.
  import type { AlbumSummary } from "../lib/ipc";
  import { albumKind, coverLarge, year } from "../lib/format";
  import { go } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";

  let { album, context }: { album: AlbumSummary; context: string } = $props();

  const large = $derived(coverLarge(album));
</script>

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

<style>
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

  /* Atelier : couverture de magazine, sans flou, couleur en aplat. */
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

  /* Android (#24, #184) : bandeau bas, pochette à droite du texte, pour voir aussi la première
     ligne de l'accueil en entier. Titre limité à deux lignes. */
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
