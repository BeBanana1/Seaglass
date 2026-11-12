<!-- Palette de commandes (#412, bureau) : un champ, ouvert par Ctrl+K, pour aller à une page,
     lancer une action du registre (`lib/actions.ts`) ou chercher un titre, un album, un
     artiste ou une playlist (commande `search`, comme la page Recherche). -->
<script lang="ts">
  import { tick, untrack } from "svelte";
  import { commands, type SearchResults } from "../lib/ipc";
  import { availableActions, open as openRoute, shortcutText, type Action, type ActionGroup } from "../lib/actions";
  import { rank } from "../lib/palette";
  import { artistNames, coverSmall, fullTitle, mediaSubtitle } from "../lib/format";
  import { closePalette, palette } from "../lib/stores/palette.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  /** Délai après la dernière frappe avant de chercher le contenu. */
  const SEARCH_DELAY_MS = 300;
  const MIN_SEARCH_LENGTH = 2;
  /** Résultats de contenu par type. */
  const PER_KIND = 3;
  /** Actions affichées quand une requête est tapée. */
  const MAX_ACTIONS = 8;

  type Section = ActionGroup | "tracks" | "albums" | "artists" | "playlists";

  interface Row {
    key: string;
    section: Section | null;
    label: string;
    detail?: string;
    image?: string | null;
    round?: boolean;
    shortcut?: string | null;
    run: () => unknown;
  }

  let query = $state("");
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLUListElement>();
  /** Actions proposées, relues à chaque ouverture (leurs libellés suivent l'état du lecteur). */
  let actions = $state.raw<{ action: Action; label: string; keywords?: string[] }[]>([]);
  let results = $state.raw<SearchResults | null>(null);
  let searching = $state(false);
  let previousFocus: HTMLElement | null = null;
  let request = 0;

  $effect(() => {
    if (!palette.open) return;
    untrack(() => {
      previousFocus = document.activeElement as HTMLElement | null;
      query = "";
      results = null;
      active = 0;
      actions = availableActions().map((action) => ({ action, label: action.label(), keywords: action.keywords }));
    });
    void tick().then(() => input?.focus());
    return () => previousFocus?.focus?.();
  });

  // Contenu : la commande `search` 300 ms après la dernière frappe ; une réponse arrivée
  // après une requête plus récente est écartée.
  $effect(() => {
    const q = query.trim();
    const id = ++request;
    if (!palette.open || q.length < MIN_SEARCH_LENGTH) {
      results = null;
      searching = false;
      return;
    }
    searching = true;
    const timer = setTimeout(async () => {
      try {
        const found = await commands.search(q);
        if (id === request) results = found;
      } catch {
        if (id === request) results = null;
      } finally {
        if (id === request) searching = false;
      }
    }, SEARCH_DELAY_MS);
    return () => clearTimeout(timer);
  });

  const rows = $derived.by((): Row[] => {
    const q = query.trim();
    const ranked = q === "" ? actions : rank(actions, q).slice(0, MAX_ACTIONS);
    const out: Row[] = ranked.map(({ action, label }) => ({
      key: action.id,
      // Sans requête : rangées par groupe ; avec une requête, dans l'ordre du classement.
      section: q === "" ? action.group : null,
      label,
      detail: q === "" ? undefined : t.palette.groups[action.group],
      shortcut: shortcutText(action),
      run: () => action.run(),
    }));
    if (!results) return out;
    for (const track of results.tracks.slice(0, PER_KIND)) {
      out.push({
        key: `track-${track.id}`,
        section: "tracks",
        label: fullTitle(track),
        detail: artistNames(track),
        image: coverSmall(track.album),
        run: () => commands.playTracks([track.id], 0, false, { kind: "search", query: q }),
      });
    }
    for (const album of results.albums.slice(0, PER_KIND)) {
      out.push({
        key: `album-${album.id}`,
        section: "albums",
        label: album.title,
        detail: mediaSubtitle({ kind: "album", ...album }),
        image: album.coverUrl,
        run: () => openRoute({ name: "album", id: album.id }),
      });
    }
    for (const artist of results.artists.slice(0, PER_KIND)) {
      out.push({
        key: `artist-${artist.id}`,
        section: "artists",
        label: artist.name,
        image: artist.pictureUrl,
        round: true,
        run: () => openRoute({ name: "artist", id: artist.id }),
      });
    }
    for (const playlist of results.playlists.slice(0, PER_KIND)) {
      out.push({
        key: `playlist-${playlist.uuid}`,
        section: "playlists",
        label: playlist.title,
        detail: mediaSubtitle({ kind: "playlist", ...playlist }),
        image: playlist.imageUrl,
        run: () => openRoute({ name: "playlist", uuid: playlist.uuid }),
      });
    }
    return out;
  });

  // La liste change sous la sélection : on repart de la première ligne.
  $effect(() => {
    void rows;
    active = 0;
  });

  function choose(row: Row) {
    closePalette();
    try {
      const result = row.run();
      if (result instanceof Promise) result.catch((e) => pushToast({ level: "error", message: String(e) }));
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function move(step: number) {
    if (rows.length === 0) return;
    active = (active + step + rows.length) % rows.length;
    void tick().then(() => list?.querySelector(`[data-index="${active}"]`)?.scrollIntoView({ block: "nearest" }));
  }

  function onKeydown(e: KeyboardEvent) {
    switch (e.key) {
      case "Escape":
        e.stopPropagation();
        closePalette();
        break;
      case "ArrowDown":
        e.preventDefault();
        move(1);
        break;
      case "ArrowUp":
        e.preventDefault();
        move(-1);
        break;
      case "Enter":
        e.preventDefault();
        if (rows[active]) choose(rows[active]);
        break;
      case "Tab":
        // Le focus reste dans le champ : la liste se parcourt aux flèches.
        e.preventDefault();
        break;
    }
  }
</script>

{#if palette.open}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="palette-backdrop" onclick={(e) => e.target === e.currentTarget && closePalette()}>
    <div class="command-palette" role="dialog" aria-modal="true" aria-label={t.palette.label}>
      <input
        bind:this={input}
        bind:value={query}
        class="palette-input"
        type="text"
        role="combobox"
        aria-expanded="true"
        aria-controls="palette-list"
        aria-activedescendant={rows[active] ? `palette-row-${active}` : undefined}
        aria-autocomplete="list"
        placeholder={t.palette.placeholder}
        spellcheck="false"
        autocomplete="off"
        onkeydown={onKeydown}
      />
      <ul class="palette-list" id="palette-list" role="listbox" bind:this={list}>
        {#each rows as row, index (row.key)}
          {#if row.section && row.section !== rows[index - 1]?.section}
            <li class="palette-section" role="presentation">{t.palette.groups[row.section]}</li>
          {/if}
          <li
            id="palette-row-{index}"
            class="palette-row"
            class:active={index === active}
            role="option"
            aria-selected={index === active}
            data-index={index}
            onmousemove={() => (active = index)}
            onclick={() => choose(row)}
          >
            {#if row.image !== undefined}
              {#if row.image}
                <img class="palette-thumb" class:round={row.round} src={row.image} alt="" loading="lazy" />
              {:else}
                <span class="palette-thumb" class:round={row.round}></span>
              {/if}
            {/if}
            <span class="palette-text">
              <span class="palette-label">{row.label}</span>
              {#if row.detail}<span class="palette-detail">{row.detail}</span>{/if}
            </span>
            {#if row.shortcut}<kbd>{row.shortcut}</kbd>{/if}
          </li>
        {/each}
        {#if rows.length === 0 && !searching}
          <li class="palette-empty" role="presentation">{t.palette.empty}</li>
        {/if}
        {#if searching}
          <li class="palette-empty" role="presentation">{t.palette.searching}</li>
        {/if}
      </ul>
      <p class="palette-hint">{t.palette.hint}</p>
    </div>
  </div>
{/if}

<style>
  .palette-backdrop {
    position: fixed;
    inset: 0;
    z-index: 95;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding: 12vh 24px 24px;
    background: rgb(0 0 0 / 0.4);
  }

  .command-palette {
    display: flex;
    flex-direction: column;
    width: min(620px, 100%);
    max-height: min(560px, 76vh);
    overflow: hidden;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
  }

  .palette-input {
    width: 100%;
    padding: 16px 18px;
    border: none;
    border-bottom: 1px solid var(--border);
    color: var(--text);
    background: transparent;
    font: inherit;
    font-size: 16px;
    outline: none;
  }

  .palette-list {
    flex: 1;
    min-height: 0;
    margin: 0;
    padding: 6px;
    overflow-y: auto;
    list-style: none;
  }

  .palette-section {
    padding: 10px 10px 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .palette-row {
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 36px;
    padding: 6px 10px;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .palette-row.active {
    background: var(--bg-hover);
  }

  .palette-thumb {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 4px;
    object-fit: cover;
    background: var(--bg-raised);
  }

  .palette-thumb.round {
    border-radius: 50%;
  }

  .palette-text {
    display: flex;
    flex: 1;
    min-width: 0;
    align-items: baseline;
    gap: 8px;
  }

  .palette-label,
  .palette-detail {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .palette-label {
    flex: 0 1 auto;
  }

  .palette-detail {
    flex: 1 1 0;
    min-width: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  kbd {
    flex: none;
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 10px;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .palette-empty {
    padding: 14px 10px;
    color: var(--text-muted);
  }

  .palette-hint {
    margin: 0;
    padding: 8px 18px;
    border-top: 1px solid var(--border);
    font-size: 11px;
    color: var(--text-muted);
  }
</style>
