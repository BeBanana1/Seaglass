<script lang="ts">
  // « Ajouter à une playlist » (#4) : vos playlists, filtre, création d'une nouvelle.
  // Ouverte par `openPlaylistPicker` (lib/stores/playlistPicker.svelte.ts).
  import { commands, type PlaylistSummary } from "../lib/ipc";
  import { plural } from "../lib/format";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import { closePlaylistPicker, notePlaylistChange, picker } from "../lib/stores/playlistPicker.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";

  let playlists = $state<PlaylistSummary[] | null>(null);
  let error = $state<string | null>(null);
  let query = $state("");
  let creating = $state(false);
  let newTitle = $state("");
  let busy = $state(false);
  let filterInput = $state<HTMLInputElement>();
  let titleInput = $state<HTMLInputElement>();

  // Rechargées à chaque ouverture : un titre ajouté change les compteurs.
  $effect(() => {
    if (!picker.request) return;
    playlists = null;
    error = null;
    query = "";
    creating = false;
    newTitle = "";
    commands
      .ownPlaylists()
      .then((p) => (playlists = p))
      .catch((e) => (error = String(e)));
    requestAnimationFrame(() => filterInput?.focus());
  });

  $effect(() => {
    if (creating) requestAnimationFrame(() => titleInput?.focus());
  });

  const shown = $derived.by(() => {
    const q = query.trim().toLocaleLowerCase("fr");
    const all = playlists ?? [];
    return q ? all.filter((p) => p.title.toLocaleLowerCase("fr").includes(q)) : all;
  });

  function done(uuid: string) {
    notePlaylistChange(uuid);
    // Les compteurs de la bibliothèque ont changé (et la liste, après une création).
    if (library.playlists.items) loadLibrary("playlists", true);
    closePlaylistPicker();
  }

  async function add(playlist: PlaylistSummary) {
    const request = picker.request;
    if (!request || busy) return;
    busy = true;
    try {
      const added = await commands.addToPlaylist(playlist.uuid, request.trackIds);
      const skipped = request.trackIds.length - added;
      const where = `« ${playlist.title} »`;
      let message: string;
      if (added === 0) message = `Déjà dans ${where}`;
      else if (request.trackIds.length === 1) message = `Ajouté à ${where}`;
      else {
        message = `${plural(added, "titre")} ${added > 1 ? "ajoutés" : "ajouté"} à ${where}`;
        if (skipped > 0) message += ` (${skipped} déjà ${skipped > 1 ? "présents" : "présent"})`;
      }
      pushToast({ level: "info", message });
      done(playlist.uuid);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    } finally {
      busy = false;
    }
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const request = picker.request;
    const title = newTitle.trim();
    if (!request || !title || busy) return;
    busy = true;
    try {
      const uuid = await commands.createPlaylist(title, request.trackIds);
      pushToast({ level: "info", message: `Playlist « ${title} » créée` });
      done(uuid);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    } finally {
      busy = false;
    }
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      if (creating) creating = false;
      else closePlaylistPicker();
    }
  }
</script>

{#if picker.request}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="picker-backdrop" onclick={(e) => e.target === e.currentTarget && closePlaylistPicker()}>
    <div
      class="playlist-picker"
      role="dialog"
      aria-modal="true"
      aria-labelledby="picker-title"
      tabindex="-1"
      onkeydown={onKeydown}
    >
      <header>
        <div>
          <h3 id="picker-title" class="display">Ajouter à une playlist</h3>
          <p class="sub">{picker.request.label}</p>
        </div>
        <button class="close" onclick={closePlaylistPicker} aria-label="Fermer"><Icon name="close" /></button>
      </header>

      <input
        class="picker-filter"
        type="text"
        placeholder="Filtrer vos playlists"
        bind:value={query}
        bind:this={filterInput}
        aria-label="Filtrer vos playlists"
      />

      {#if creating}
        <form class="picker-new-form" onsubmit={create}>
          <input
            type="text"
            placeholder="Nom de la playlist"
            maxlength="200"
            bind:value={newTitle}
            bind:this={titleInput}
            aria-label="Nom de la nouvelle playlist"
          />
          <button class="primary" type="submit" disabled={busy || !newTitle.trim()}>Créer</button>
        </form>
      {:else}
        <button class="picker-item picker-new" onclick={() => (creating = true)} disabled={busy}>
          <span class="thumb"><Icon name="add" /></span>
          <span class="name">Nouvelle playlist…</span>
        </button>
      {/if}

      <div class="list" aria-busy={busy}>
        {#if error}
          <p class="error">{error}</p>
        {:else if !playlists}
          <p class="sub">Chargement…</p>
        {:else if playlists.length === 0}
          <p class="sub">Vous n'avez pas encore de playlist.</p>
        {:else if shown.length === 0}
          <p class="sub">Aucune playlist ne correspond à « {query.trim()} ».</p>
        {:else}
          {#each shown as playlist (playlist.uuid)}
            <button class="picker-item" onclick={() => add(playlist)} disabled={busy}>
              <span class="thumb">
                {#if playlist.imageUrl}<img src={playlist.imageUrl} alt="" loading="lazy" />{:else}<Icon
                    name="queue"
                  />{/if}
              </span>
              <span class="name">{playlist.title}</span>
              {#if playlist.numberOfTracks !== null}
                <span class="count">{plural(playlist.numberOfTracks, "titre")}</span>
              {/if}
            </button>
          {/each}
        {/if}
      </div>
    </div>
  </div>
{/if}

<style>
  .picker-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.45);
  }

  .playlist-picker {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: min(440px, 100%);
    max-height: min(620px, 100%);
    padding: 18px;
    border: 1px solid var(--border);
    border-radius: calc(var(--panel-radius) + 2px);
    color: var(--text);
    background: var(--dialog-bg);
    box-shadow: 0 24px 60px rgb(0 0 0 / 0.4);
  }

  .playlist-picker:focus {
    outline: none;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  h3 {
    margin: 0;
    font-size: 20px;
  }

  .sub {
    margin: 2px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .close {
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .close:hover {
    color: var(--text);
  }

  input {
    width: 100%;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-raised);
    font: inherit;
  }

  .picker-new-form {
    display: flex;
    gap: 8px;
  }

  .picker-new-form .primary {
    padding: 0 16px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .picker-new-form .primary:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .list {
    display: flex;
    flex-direction: column;
    min-height: 120px;
    overflow-y: auto;
  }

  .picker-item {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px;
    border-radius: var(--radius);
    text-align: left;
  }

  .picker-item:hover:not(:disabled),
  .picker-item:focus-visible {
    background: var(--bg-hover);
  }

  .picker-item:disabled {
    cursor: progress;
  }

  .thumb {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 40px;
    height: 40px;
    overflow: hidden;
    border-radius: 4px;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .thumb img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .picker-new .thumb {
    color: var(--accent);
  }

  .name {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .count {
    flex-shrink: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }

  :global([data-theme="studio"]) .playlist-picker,
  :global([data-theme="studio"]) input,
  :global([data-theme="studio"]) .picker-new-form .primary {
    border-radius: 0;
  }
</style>
