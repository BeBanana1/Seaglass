<script lang="ts">
  import { tick } from "svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { setListSearch } from "../lib/stores/listSearch.svelte";
  import { openMenu, type MenuEntry } from "../lib/stores/menu.svelte";
  import Icon from "./Icon.svelte";
  import type { SortKey, TrackSort } from "../lib/trackOrder";

  let {
    query = $bindable(""),
    sort = $bindable(),
    keys,
    shown,
    total,
  }: {
    /** Filter text (title, artists, album). */
    query: string;
    sort: TrackSort;
    /** Offered criteria, in the menu order. */
    keys: SortKey[];
    /** Number of tracks displayed after filtering, and total number. */
    shown: number;
    total: number;
  } = $props();

  let root = $state<HTMLElement>();
  let input = $state<HTMLInputElement>();

  // Android (#151): search mode as soon as the field is focused (the keyboard is about to open),
  // left only by Cancel or Back. Leaving it on focus loss would move the page under the finger
  // between the tap and the click: the click would land on the header that came back.
  let searching = $state(false);

  function enter() {
    if (!platform.mobile || searching) return;
    searching = true;
    // The page hides its header on the next render: the field then moves up to the top.
    tick().then(() => root?.scrollIntoView({ block: "start" }));
  }

  function leave() {
    query = "";
    searching = false;
    input?.blur();
  }

  $effect(() => {
    setListSearch(searching, leave);
    return () => setListSearch(false, null);
  });

  function setKey(e: Event) {
    sort = { ...sort, key: (e.currentTarget as HTMLSelectElement).value as SortKey };
  }

  // Android (#182): a single sort button, which opens the app's menu (criteria, then direction).
  function openSortMenu(e: MouseEvent) {
    const button = e.currentTarget as HTMLElement;
    const rect = button.getBoundingClientRect();
    const entries: MenuEntry[] = [
      ...keys.map((key) => ({
        label: t.library.sort[key],
        checked: sort.key === key,
        action: () => (sort = { ...sort, key }),
      })),
      "separator",
      {
        label: t.library.ascending,
        checked: !sort.descending,
        action: () => (sort = { ...sort, descending: false }),
      },
      {
        label: t.library.descending,
        checked: sort.descending,
        action: () => (sort = { ...sort, descending: true }),
      },
    ];
    openMenu({ x: rect.right, y: rect.bottom + 4, entries, from: button });
  }
</script>

<div class="track-tools" class:searching bind:this={root}>
  <!-- Escape clears the field (native behavior of a search field). -->
  {#if platform.mobile}<span class="filter-icon"><Icon name="search" size={18} /></span>{/if}
  <input
    type="search"
    class="filter"
    placeholder={t.library.filter}
    aria-label={t.library.filter}
    bind:value={query}
    bind:this={input}
    onfocus={enter}
    autocomplete="off"
    spellcheck="false"
  />
  {#if searching}
    <!-- `mousedown` has no effect: the field keeps the focus until the click. -->
    <button class="search-cancel" onmousedown={(e) => e.preventDefault()} onclick={leave}>
      {t.common.cancel}
    </button>
  {/if}
  {#if query.trim()}
    <span class="count">{t.common.tracksOf(shown, total)}</span>
  {/if}
  {#if !searching && platform.mobile}
    <button
      class="sort-button"
      onclick={openSortMenu}
      title={t.library.sortBy}
      aria-label={t.library.sortBy}
    >
      <Icon name="sort" size={20} />
    </button>
  {:else if !searching}
    <div class="sort">
      <select value={sort.key} onchange={setKey} aria-label={t.library.sortBy}>
        {#each keys as key (key)}
          <option value={key}>{t.library.sort[key]}</option>
        {/each}
      </select>
      <button
        class="direction"
        onclick={() => (sort = { ...sort, descending: !sort.descending })}
        title={sort.descending ? t.library.descending : t.library.ascending}
        aria-label={sort.descending ? t.library.descending : t.library.ascending}
      >
        {sort.descending ? "↓" : "↑"}
      </button>
    </div>
  {/if}
</div>

<style>
  .track-tools {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 10px;
    margin: 0 10px 12px;
  }

  .filter {
    flex: 0 1 280px;
    min-width: 160px;
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg-raised);
    user-select: text;
  }

  .filter::placeholder {
    color: var(--text-muted);
  }

  .count {
    font-size: 13px;
    color: var(--text-muted);
  }

  .sort {
    display: flex;
    align-items: center;
    gap: 4px;
    margin-left: auto;
  }

  select,
  .direction {
    height: 32px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--text-muted);
    background: var(--bg-raised);
    font: inherit;
    font-size: 13px;
    clip-path: var(--control-clip);
  }

  select {
    padding: 0 10px;
    cursor: pointer;
  }

  /* Dropdown list: opaque background, whatever the theme. */
  option {
    color: var(--text);
    background: var(--dialog-bg);
  }

  .direction {
    width: 32px;
  }

  select:hover,
  .direction:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  /* Android (#151): field and Cancel on the first row, counter below. */
  .searching .filter {
    flex: 1 1 0;
    min-width: 0;
  }

  .searching .count {
    flex-basis: 100%;
    padding-left: 14px;
  }

  .search-cancel {
    padding: 6px 4px;
    border: none;
    color: var(--accent);
    background: none;
    font: inherit;
    font-size: 14px;
    font-weight: 600;
  }

  /* Android (#182): filter at full width with a magnifier, sort as a single button. */
  :global(:root[data-platform="android"]) .track-tools {
    position: relative;
    flex-wrap: nowrap;
  }

  :global(:root[data-platform="android"]) .track-tools.searching {
    flex-wrap: wrap;
  }

  :global(:root[data-platform="android"]) .filter {
    flex: 1 1 0;
    min-width: 0;
    height: 44px;
    padding: 0 14px 0 40px;
    font-size: 15px;
  }

  .filter-icon {
    position: absolute;
    left: 14px;
    top: 13px;
    color: var(--text-muted);
    pointer-events: none;
  }

  .sort-button {
    display: flex;
    flex-shrink: 0;
    align-items: center;
    justify-content: center;
    width: 44px;
    height: 44px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--text-muted);
    background: var(--bg-raised);
    clip-path: var(--control-clip);
  }

  .sort-button:active {
    color: var(--text);
    background: var(--bg-hover);
  }

  :global([data-theme="studio"]) .filter {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .filter {
    border-radius: 0;
    border-width: 0 0 1px;
    border-color: var(--text);
    background: transparent;
  }
</style>
