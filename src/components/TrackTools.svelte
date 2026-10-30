<script lang="ts">
  import { num, t } from "../lib/i18n.svelte";
  import type { SortKey, TrackSort } from "../lib/trackOrder";

  let {
    query = $bindable(""),
    sort = $bindable(),
    keys,
    shown,
    total,
  }: {
    /** Texte du filtre (titre, artistes, album). */
    query: string;
    sort: TrackSort;
    /** Critères proposés, dans l'ordre du menu. */
    keys: SortKey[];
    /** Nombre de titres affichés après filtrage, et nombre total. */
    shown: number;
    total: number;
  } = $props();

  function setKey(e: Event) {
    sort = { ...sort, key: (e.currentTarget as HTMLSelectElement).value as SortKey };
  }
</script>

<div class="track-tools">
  <!-- Échap vide le champ (comportement natif d'un champ de recherche). -->
  <input
    type="search"
    class="filter"
    placeholder={t.library.filter}
    aria-label={t.library.filter}
    bind:value={query}
    autocomplete="off"
    spellcheck="false"
  />
  {#if query.trim()}
    <span class="count">{t.common.tracksOf(shown, total)}</span>
  {/if}
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
      >{sort.descending ? "↓" : "↑"}</button
    >
  </div>
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

  /* Liste déroulante : fond opaque, quel que soit le thème. */
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
