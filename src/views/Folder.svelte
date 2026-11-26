<script lang="ts">
  // Playlist folder (#331): its playlists as cards, Rename and Delete.
  // The playlists come from the library (Playlists tab), filtered by the folder.
  import type { MediaItem } from "../lib/ipc";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import {
    deleteFolder,
    folderById,
    folderPlaylists,
    folders,
    loadFolders,
    renameFolder,
  } from "../lib/stores/folders.svelte";
  import { back } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import CardGrid from "../components/CardGrid.svelte";
  import Icon from "../components/Icon.svelte";

  let { id }: { id: string } = $props();

  let renaming = $state(false);
  let name = $state("");
  let confirming = $state(false);
  let busy = $state(false);
  let nameInput = $state<HTMLInputElement>();

  $effect(() => {
    loadLibrary("playlists");
    loadFolders();
  });

  $effect(() => {
    void id;
    renaming = false;
    confirming = false;
  });

  const folder = $derived(folderById(id));
  const cards = $derived<MediaItem[]>(
    folder
      ? folderPlaylists(folder, library.playlists.items ?? []).map((p) => ({
          kind: "playlist",
          ...p,
        }))
      : [],
  );

  function startRename() {
    if (!folder) return;
    name = folder.name;
    confirming = false;
    renaming = true;
    requestAnimationFrame(() => nameInput?.select());
  }

  async function saveName(e: SubmitEvent) {
    e.preventDefault();
    const clean = name.trim();
    if (!folder || !clean || busy) return;
    if (clean === folder.name) {
      renaming = false;
      return;
    }
    busy = true;
    if (await renameFolder(folder.id, clean)) renaming = false;
    busy = false;
  }

  async function remove() {
    if (!folder || busy) return;
    busy = true;
    if (await deleteFolder(folder.id)) back();
    busy = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      renaming = false;
    }
  }
</script>

<section class="folder-page">
  {#if !folder}
    {#if folders.items && !folders.loading}
      <p class="muted">{t.library.folders.notFound}</p>
    {:else}
      <p class="muted">{t.common.loading}</p>
    {/if}
  {:else}
    <header class="folder-header">
      <span class="folder-icon"><Icon name="folder" size={40} /></span>
      <div class="info">
        {#if renaming}
          <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
          <form class="folder-rename" onsubmit={saveName} onkeydown={onKeydown}>
            <input
              type="text"
              maxlength="200"
              bind:value={name}
              bind:this={nameInput}
              aria-label={t.library.folders.renameLabel}
            />
            <button class="primary" type="submit" disabled={busy || !name.trim()}>
              {t.library.folders.save}
            </button>
            <button type="button" class="pill" onclick={() => (renaming = false)}>
              {t.library.folders.cancel}
            </button>
          </form>
        {:else}
          <h1>{folder.name}</h1>
        {/if}
        <p class="meta">{t.library.folders.count(cards.length)}</p>
        {#if !renaming}
          <div class="folder-actions">
            <button class="pill" onclick={startRename} disabled={busy}>
              <Icon name="edit" size={14} />
              {t.library.folders.rename}
            </button>
            {#if confirming}
              <span class="confirm" role="alert">{t.library.folders.deleteConfirm}</span>
              <button class="pill danger" onclick={remove} disabled={busy}>
                {t.library.folders.deleteYes}
              </button>
              <button class="pill" onclick={() => (confirming = false)} disabled={busy}>
                {t.library.folders.cancel}
              </button>
            {:else}
              <button class="pill" onclick={() => (confirming = true)} disabled={busy}>
                {t.library.folders.delete}
              </button>
            {/if}
          </div>
        {/if}
      </div>
    </header>

    {#if library.playlists.error}
      <p class="error">{library.playlists.error}</p>
    {:else if !library.playlists.items}
      <p class="muted">{t.common.loading}</p>
    {:else if cards.length === 0}
      <p class="muted">{t.library.folders.empty}</p>
    {:else}
      <CardGrid items={cards} />
    {/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .folder-header {
    display: flex;
    align-items: center;
    gap: 18px;
    margin: 0 10px 24px;
  }

  .folder-icon {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 88px;
    height: 88px;
    border-radius: var(--radius-card);
    color: var(--accent);
    background: var(--bg-raised);
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: 26px;
    overflow-wrap: anywhere;
  }

  .meta {
    margin: 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .folder-actions,
  .folder-rename {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .pill {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
  }

  .pill:hover:not(:disabled) {
    color: var(--text);
  }

  .pill.danger {
    border-color: var(--danger);
    color: var(--danger);
  }

  .confirm {
    font-size: 13px;
  }

  input {
    min-width: 0;
    width: min(320px, 100%);
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-raised);
    font: inherit;
    font-size: 18px;
  }

  .primary {
    padding: 6px 16px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:disabled {
    opacity: 0.5;
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }

  :global(:root[data-platform="android"]) .folder-icon {
    width: 64px;
    height: 64px;
  }
</style>
