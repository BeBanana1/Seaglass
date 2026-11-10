<script lang="ts">
  // Mode « Personnaliser l'accueil » (#21, #22) : une ligne par section, avec Monter /
  // Descendre et Masquer / Afficher. Tout se fait avec des boutons, donc au clavier ; sur
  // le bureau, une ligne se déplace aussi par glisser-déposer (#246, comme la file) ; sur
  // Android, par un appui long puis en glissant le doigt (#290).
  // Le composant ne fait que réordonner la liste affichée : l'enregistrement et la règle
  // des sections absentes ou nouvelles sont côté backend (`HomeLayout`).
  import { tick } from "svelte";
  import type { HomeSection } from "../lib/ipc";
  import { mediaImage } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { Reorder } from "../lib/reorder.svelte";
  import { touchReorder } from "../lib/touch-reorder";
  import Icon from "./Icon.svelte";

  let {
    sections,
    canReset,
    featuredHidden,
    onchange,
    onfeatured,
    onreset,
  }: {
    sections: HomeSection[];
    /** Faux si la disposition est déjà celle par défaut. */
    canReset: boolean;
    /** Bandeau « À la une » masqué (#185) : toujours en tête, il ne se déplace pas. */
    featuredHidden: boolean;
    onchange: (next: HomeSection[]) => void;
    onfeatured: (shown: boolean) => void;
    onreset: () => void;
  } = $props();

  // Vignettes du bandeau : les premiers albums des sections affichées, d'où il est tiré.
  const featuredImages = $derived(
    sections
      .filter((s) => !s.hidden)
      .flatMap((s) => s.items.filter((i) => i.kind === "album"))
      .map(mediaImage)
      .filter((i): i is string => !!i)
      .slice(0, 3),
  );

  // Deux lignes peuvent partager une clé (backend : groupées) : une seule entrée ici.
  const groups = $derived.by(() => {
    const seen = new Map<string, { key: string; title: string; hidden: boolean; count: number; images: string[] }>();
    for (const s of sections) {
      const g = seen.get(s.key);
      if (g) {
        g.count += s.items.length;
        continue;
      }
      const images = s.items
        .map(mediaImage)
        .filter((i): i is string => !!i)
        .slice(0, 3);
      seen.set(s.key, { key: s.key, title: s.title, hidden: s.hidden, count: s.items.length, images });
    }
    return [...seen.values()];
  });

  let list = $state<HTMLOListElement>();
  let announce = $state("");

  /** Place la section `index` en position `target` (les autres glissent d'un cran). */
  function moveTo(index: number, target: number): string | null {
    const keys = groups.map((g) => g.key);
    if (target < 0 || target >= keys.length || target === index) return null;
    const [key] = keys.splice(index, 1);
    keys.splice(target, 0, key);
    onchange(keys.flatMap((k) => sections.filter((s) => s.key === k)));
    announce = t.views.homeEditor.moved(groups[index].title, target + 1, keys.length);
    return key;
  }

  async function move(index: number, dir: -1 | 1) {
    const key = moveTo(index, index + dir);
    if (key === null) return;
    // Le nœud déplacé perd le focus : on le rend au même bouton, ou à l'autre s'il est désactivé.
    await tick();
    const sel = (d: string) => `[data-key="${CSS.escape(key)}"] [data-dir="${d}"]`;
    const btn =
      list?.querySelector<HTMLButtonElement>(`${sel(dir < 0 ? "up" : "down")}:not(:disabled)`) ??
      list?.querySelector<HTMLButtonElement>(sel(dir < 0 ? "down" : "up"));
    btn?.focus();
  }

  const reorder = new Reorder((from, to) => void moveTo(from, to));

  /** Lignes réorganisables, dans l'ordre (sans le bandeau « À la une »). */
  const items = () => Array.from(list?.querySelectorAll<HTMLElement>(".home-editor-item.draggable") ?? []);

  function toggleFeatured() {
    announce = t.views.homeEditor.featuredToggled(!featuredHidden);
    onfeatured(featuredHidden);
  }

  function toggle(key: string) {
    const g = groups.find((g) => g.key === key);
    if (g) announce = t.views.homeEditor.toggled(g.title, !g.hidden);
    onchange(sections.map((s) => (s.key === key ? { ...s, hidden: !s.hidden } : s)));
  }
</script>

<div class="home-editor">
  <p class="home-editor-hint">{t.views.homeEditor.hint}</p>
  <ol
    class="home-editor-list"
    bind:this={list}
    aria-label={t.views.homeEditor.sections}
    ondragleave={(e) => reorder.dragleave(e, list)}
  >
    <li class="home-editor-item" class:off={featuredHidden} data-key="featured">
      <span class="home-editor-thumbs" aria-hidden="true">
        {#each featuredImages as src (src)}<img {src} alt="" loading="lazy" />{/each}
      </span>
      <span class="home-editor-name">
        <span class="home-editor-title">{t.views.homeEditor.featured}</span>
        <span class="home-editor-meta">
          {t.views.homeEditor.featuredMeta}{featuredHidden ? t.views.homeEditor.featuredHiddenSuffix : ""}
        </span>
      </span>
      <span class="home-editor-actions">
        <!-- Pas de Monter / Descendre : le bandeau reste en tête. Place gardée pour l'alignement. -->
        <span class="home-editor-spacer" aria-hidden="true"></span>
        <button
          class="home-editor-toggle"
          role="switch"
          aria-checked={!featuredHidden}
          aria-label={t.views.homeEditor.showItem(t.views.homeEditor.featured)}
          onclick={toggleFeatured}
        >
          <Icon name={featuredHidden ? "eyeOff" : "eye"} size={16} />
          <span class="home-editor-toggle-label">
            {featuredHidden ? t.views.homeEditor.featuredHidden : t.views.homeEditor.featuredShown}
          </span>
        </button>
      </span>
    </li>
    {#each groups as g, i (g.key)}
      <!-- Le glisser double les boutons Monter / Descendre, qui restent pour le clavier. -->
      <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
      <li
        class="home-editor-item draggable"
        class:off={g.hidden}
        class:dragging={reorder.from === i}
        class:drop-before={reorder.marker(i)}
        class:drop-after={i === groups.length - 1 && reorder.marker(i + 1)}
        data-key={g.key}
        draggable={platform.mobile ? undefined : "true"}
        ondragstart={(e) => reorder.dragstart(e, i)}
        ondragover={(e) => reorder.dragover(e, i)}
        ondrop={(e) => reorder.drop(e)}
        ondragend={() => reorder.end()}
        use:touchReorder={{ enabled: platform.mobile, index: i, reorder, items }}
      >
        <span class="home-editor-thumbs" aria-hidden="true">
          {#each g.images as src (src)}<img {src} alt="" loading="lazy" />{/each}
        </span>
        <span class="home-editor-name">
          <span class="home-editor-title">{g.title}</span>
          <span class="home-editor-meta">
            {t.views.homeEditor.items(g.count)}{g.hidden ? t.views.homeEditor.hiddenSuffix : ""}
          </span>
        </span>
        <span class="home-editor-actions">
          <button
            class="home-editor-icon"
            data-dir="up"
            disabled={i === 0}
            onclick={() => move(i, -1)}
            aria-label={t.views.homeEditor.upItem(g.title)}
            title={t.views.homeEditor.up}
          >
            <Icon name="up" />
          </button>
          <button
            class="home-editor-icon"
            data-dir="down"
            disabled={i === groups.length - 1}
            onclick={() => move(i, 1)}
            aria-label={t.views.homeEditor.downItem(g.title)}
            title={t.views.homeEditor.down}
          >
            <Icon name="down" />
          </button>
          <button
            class="home-editor-toggle"
            role="switch"
            aria-checked={!g.hidden}
            aria-label={t.views.homeEditor.showItem(g.title)}
            onclick={() => toggle(g.key)}
          >
            <Icon name={g.hidden ? "eyeOff" : "eye"} size={16} />
            <span class="home-editor-toggle-label">
              {g.hidden ? t.views.homeEditor.hidden : t.views.homeEditor.shown}
            </span>
          </button>
        </span>
      </li>
    {/each}
  </ol>
  <div class="home-editor-footer">
    <button class="home-editor-reset" disabled={!canReset} onclick={onreset}>
      {t.views.homeEditor.reset}
    </button>
  </div>
  <p class="visually-hidden" aria-live="polite">{announce}</p>
</div>

<style>
  .home-editor {
    margin: 0 10px;
  }

  .home-editor-hint {
    margin: 0 0 16px;
    color: var(--text-muted);
  }

  .home-editor-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .home-editor-item {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .home-editor-item.draggable {
    position: relative;
  }

  :global(:root:not([data-platform="android"])) .home-editor-item.draggable {
    cursor: grab;
  }

  .home-editor-item :global(img) {
    -webkit-user-drag: none;
  }

  .home-editor-item.dragging:not(:global(.touch-dragging)) {
    opacity: 0.4;
  }

  /* Ligne saisie au doigt (#290) : elle suit le doigt, au-dessus des autres. */
  .home-editor-item:global(.touch-dragging) {
    z-index: 1;
    transform: translateY(var(--drag-y, 0px)) scale(1.02);
    /* Fond opaque : la surface des thèmes est translucide, le texte dessous transparaissait. */
    background: linear-gradient(var(--bg-raised), var(--bg-raised)), var(--bg);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }

  /* Repère d'insertion, comme dans la file (`QueueList`). */
  .home-editor-item.drop-before::before,
  .home-editor-item.drop-after::after {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    height: 2px;
    border-radius: 1px;
    background: var(--accent);
    pointer-events: none;
  }

  .home-editor-item.drop-before::before {
    top: -5px;
  }

  .home-editor-item.drop-after::after {
    bottom: -5px;
  }

  .home-editor-thumbs {
    display: flex;
    flex-shrink: 0;
    width: 84px;
  }

  .home-editor-thumbs img {
    width: 40px;
    height: 40px;
    margin-right: -18px;
    object-fit: cover;
    border: 2px solid var(--surface);
    border-radius: calc(var(--radius) * 0.6);
    background: var(--bg-hover);
  }

  .home-editor-name {
    display: flex;
    flex: 1;
    flex-direction: column;
    min-width: 0;
  }

  .home-editor-title {
    overflow: hidden;
    font-weight: 600;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .home-editor-meta {
    font-size: 12px;
    color: var(--text-muted);
  }

  .home-editor-item.off .home-editor-thumbs,
  .home-editor-item.off .home-editor-title {
    opacity: 0.45;
  }

  .home-editor-item.off .home-editor-thumbs {
    filter: grayscale(1);
  }

  .home-editor-actions {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .home-editor-spacer {
    width: 68px;
  }

  .home-editor-icon {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 999px;
    color: var(--text-muted);
  }

  .home-editor-icon:hover:not(:disabled),
  .home-editor-toggle:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  .home-editor-icon:disabled {
    opacity: 0.3;
    cursor: default;
  }

  .home-editor-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    width: 116px;
    margin-left: 6px;
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    font-size: 13px;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    box-shadow: var(--shadow-raised);
  }

  .home-editor-toggle[aria-checked="true"] {
    color: var(--accent);
  }

  .home-editor-toggle[aria-checked="false"] {
    color: var(--text-muted);
  }

  .home-editor-footer {
    display: flex;
    justify-content: flex-end;
    margin-top: 14px;
  }

  .home-editor-reset {
    padding: 8px 18px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    font-weight: 600;
    letter-spacing: var(--label-tracking);
    text-transform: var(--label-transform);
    background: var(--control-bg, none);
    box-shadow: var(--shadow-raised);
  }

  .home-editor-reset:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .home-editor-reset:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .visually-hidden {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip-path: inset(50%);
    white-space: nowrap;
  }

  /* Téléphone (#135) : vignettes et libellé de la bascule ne laissaient plus de place au
     titre et poussaient les boutons hors du cadre. Icônes seules, à la taille du doigt. */
  :global(:root[data-platform="android"]) .home-editor {
    margin: 0;
  }

  :global(:root[data-platform="android"]) .home-editor-item {
    gap: 8px;
    padding: 6px 6px 6px 12px;
    /* Appui long = saisie de la ligne, pas sélection de texte ni menu de la WebView. */
    user-select: none;
    -webkit-user-select: none;
    -webkit-touch-callout: none;
  }

  :global(:root[data-platform="android"]) .home-editor-thumbs,
  :global(:root[data-platform="android"]) .home-editor-toggle-label {
    display: none;
  }

  :global(:root[data-platform="android"]) .home-editor-actions {
    flex-shrink: 0;
    gap: 2px;
  }

  :global(:root[data-platform="android"]) .home-editor-spacer {
    width: 82px;
  }

  :global(:root[data-platform="android"]) .home-editor-icon {
    width: 40px;
    height: 40px;
  }

  :global(:root[data-platform="android"]) .home-editor-toggle {
    justify-content: center;
    width: 40px;
    height: 40px;
    margin-left: 4px;
    padding: 0;
  }

  :global([data-theme="studio"]) .home-editor-icon {
    border-radius: 0;
  }

  :global([data-theme="studio"]) .home-editor-toggle,
  :global([data-theme="studio"]) .home-editor-reset {
    font-size: 12px;
  }

  :global([data-theme="atelier"]) .home-editor-item {
    border-width: 0 0 1px;
    border-radius: 0;
    background: none;
  }

  :global([data-theme="atelier"]) .home-editor-title {
    font-family: var(--display-family);
    font-size: 17px;
  }
</style>
