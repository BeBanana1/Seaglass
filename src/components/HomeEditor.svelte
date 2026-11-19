<script lang="ts">
  // "Customize home" mode (#21, #22): one row per section, with Move up / Move down and
  // Hide / Show. Everything works with buttons, so from the keyboard; on desktop a row can
  // also be dragged and dropped (#246, like the queue); on Android, with a long press then
  // a finger drag (#290).
  // The component only reorders the shown list: saving, and the rule for missing or new
  // sections, live in the backend (`HomeLayout`).
  import { tick } from "svelte";
  import type { HomeSection, MediaItem } from "../lib/ipc";
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
    /** False when the layout is already the default one. */
    canReset: boolean;
    /** "Featured" banner hidden (#185): always first, it cannot be moved. */
    featuredHidden: boolean;
    onchange: (next: HomeSection[]) => void;
    onfeatured: (shown: boolean) => void;
    onreset: () => void;
  } = $props();

  /** First three distinct images: the thumbnails are keyed by URL, and the same album often
   *  shows up in several sections, so a duplicate would break the whole editor (#514). */
  function thumbs(items: MediaItem[]): string[] {
    const urls = items.map(mediaImage).filter((i): i is string => !!i);
    return [...new Set(urls)].slice(0, 3);
  }

  // Featured banner thumbnails: the first albums of the shown sections, which it is drawn from.
  const featuredImages = $derived(
    thumbs(
      sections.filter((s) => !s.hidden).flatMap((s) => s.items.filter((i) => i.kind === "album")),
    ),
  );

  // Two rows can share a key (grouped by the backend): a single entry here.
  const groups = $derived.by(() => {
    const seen = new Map<
      string,
      { key: string; title: string; hidden: boolean; count: number; images: string[] }
    >();
    for (const s of sections) {
      const g = seen.get(s.key);
      if (g) {
        g.count += s.items.length;
        continue;
      }
      seen.set(s.key, {
        key: s.key,
        title: s.title,
        hidden: s.hidden,
        count: s.items.length,
        images: thumbs(s.items),
      });
    }
    return [...seen.values()];
  });

  let list = $state<HTMLOListElement>();
  let announce = $state("");

  /** Moves section `index` to position `target` (the others shift by one). */
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
    // The moved node loses focus: give it back to the same button, or the other one if disabled.
    await tick();
    const sel = (d: string) => `[data-key="${CSS.escape(key)}"] [data-dir="${d}"]`;
    const btn =
      list?.querySelector<HTMLButtonElement>(`${sel(dir < 0 ? "up" : "down")}:not(:disabled)`) ??
      list?.querySelector<HTMLButtonElement>(sel(dir < 0 ? "down" : "up"));
    btn?.focus();
  }

  const reorder = new Reorder((from, to) => void moveTo(from, to));

  /** Reorderable rows, in order (without the "Featured" banner). */
  const items = () =>
    Array.from(list?.querySelectorAll<HTMLElement>(".home-editor-item.draggable") ?? []);

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
          {t.views.homeEditor.featuredMeta}{featuredHidden
            ? t.views.homeEditor.featuredHiddenSuffix
            : ""}
        </span>
      </span>
      <span class="home-editor-actions">
        <!-- No Move up / Move down: the banner stays first. Space kept for alignment. -->
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
      <!-- Dragging duplicates the Move up / Move down buttons, which stay for the keyboard. -->
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

  /* Row grabbed with a finger (#290): it follows the finger, above the others. */
  .home-editor-item:global(.touch-dragging) {
    z-index: 1;
    transform: translateY(var(--drag-y, 0px)) scale(1.02);
    /* Opaque background: theme surfaces are translucent and the text below showed through. */
    background: linear-gradient(var(--bg-raised), var(--bg-raised)), var(--bg);
    box-shadow: 0 8px 24px rgb(0 0 0 / 0.35);
  }

  /* Insertion marker, as in the queue (`QueueList`). */
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

  /* Phone (#135): thumbnails and the toggle label left no room for the title and pushed
     the buttons out of the frame. Icons only, finger-sized. */
  :global(:root[data-platform="android"]) .home-editor {
    margin: 0;
  }

  :global(:root[data-platform="android"]) .home-editor-item {
    gap: 8px;
    padding: 6px 6px 6px 12px;
    /* Long press = grabbing the row, not text selection or the WebView menu. */
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
