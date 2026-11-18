<script lang="ts">
  // « Déplacer vers un dossier… » (#331) : dossiers existants, « Hors dossier », création.
  // Ouverte par `openFolderPicker` (lib/stores/folders.svelte.ts). Reprend les classes du
  // sélecteur de playlist (`.playlist-picker`, `.picker-item`, `.picker-new-form`) : les
  // thèmes l'habillent de la même façon.
  import { t } from "../lib/i18n.svelte";
  import {
    closeFolderPicker,
    createFolder,
    folderOf,
    folderPicker,
    folders,
    movePlaylists,
  } from "../lib/stores/folders.svelte";
  import Icon from "./Icon.svelte";

  let creating = $state(false);
  let newName = $state("");
  let busy = $state(false);
  let nameInput = $state<HTMLInputElement>();
  let dialog = $state<HTMLDivElement>();

  // Sans dossier, la création est la seule chose à faire : formulaire ouvert d'emblée.
  const noFolder = $derived(folders.items?.length === 0);

  $effect(() => {
    if (!folderPicker.request) return;
    creating = false;
    newName = "";
    // Le champ du nom, s'il est ouvert d'emblée, garde le focus.
    requestAnimationFrame(() => (creating ? nameInput : dialog)?.focus());
  });

  $effect(() => {
    if (folderPicker.request && noFolder) creating = true;
  });

  $effect(() => {
    if (creating) requestAnimationFrame(() => nameInput?.focus());
  });

  // Dossier actuel : celui de la première playlist (une seule à la fois depuis le menu).
  const currentId = $derived(
    folderPicker.request ? (folderOf(folderPicker.request.uuids[0])?.id ?? null) : null,
  );

  async function moveTo(folderId: string | null) {
    const request = folderPicker.request;
    if (!request || busy) return;
    busy = true;
    if (await movePlaylists(request.uuids, folderId)) closeFolderPicker();
    busy = false;
  }

  async function create(e: SubmitEvent) {
    e.preventDefault();
    const request = folderPicker.request;
    const name = newName.trim();
    if (!request || !name || busy) return;
    busy = true;
    if (await createFolder(name, request.uuids)) closeFolderPicker();
    busy = false;
  }

  function onKeydown(e: KeyboardEvent) {
    if (e.key === "Escape") {
      e.stopPropagation();
      if (creating && !noFolder) creating = false;
      else closeFolderPicker();
    }
  }
</script>

{#if folderPicker.request}
  <!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
  <div class="picker-backdrop" onclick={(e) => e.target === e.currentTarget && closeFolderPicker()}>
    <div
      class="playlist-picker folder-picker"
      role="dialog"
      aria-modal="true"
      aria-labelledby="folder-picker-title"
      tabindex="-1"
      bind:this={dialog}
      onkeydown={onKeydown}
    >
      <header>
        <div>
          <h3 id="folder-picker-title" class="display">{t.library.folders.pickerTitle}</h3>
          <p class="sub">{folderPicker.request.label}</p>
        </div>
        <button class="close" onclick={closeFolderPicker} aria-label={t.common.close}>
          <Icon name="close" />
        </button>
      </header>

      {#if creating}
        <form class="picker-new-form" onsubmit={create}>
          <input
            type="text"
            placeholder={t.library.folders.newName}
            maxlength="200"
            bind:value={newName}
            bind:this={nameInput}
            aria-label={t.library.folders.newName}
          />
          <button class="primary" type="submit" disabled={busy || !newName.trim()}>
            {t.library.folders.create}
          </button>
        </form>
      {:else}
        <button class="picker-item picker-new" onclick={() => (creating = true)} disabled={busy}>
          <span class="thumb"><Icon name="add" /></span>
          <span class="name">{t.library.folders.newFolder}</span>
        </button>
      {/if}

      <div class="list" aria-busy={busy || folders.loading}>
        {#if !folders.items}
          <p class="sub">{t.common.loading}</p>
        {:else}
          {#if currentId}
            <button class="picker-item" onclick={() => moveTo(null)} disabled={busy}>
              <span class="thumb"><Icon name="remove" /></span>
              <span class="name">{t.library.folders.out}</span>
            </button>
          {/if}
          {#each folders.items as folder (folder.id)}
            <button
              class="picker-item"
              onclick={() => moveTo(folder.id)}
              disabled={busy || folder.id === currentId}
              aria-current={folder.id === currentId ? "true" : undefined}
            >
              <span class="thumb"><Icon name="folder" /></span>
              <span class="name">{folder.name}</span>
              {#if folder.id === currentId}
                <span class="count">{t.library.folders.current}</span>
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
    width: min(400px, 100%);
    max-height: min(560px, 100%);
    padding: 18px;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
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
    flex: 1;
    min-width: 0;
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
    cursor: default;
  }

  .picker-item[aria-current="true"] {
    opacity: 0.6;
  }

  .thumb {
    display: grid;
    place-items: center;
    flex-shrink: 0;
    width: 40px;
    height: 40px;
    border-radius: 4px;
    color: var(--text-muted);
    background: var(--bg-raised);
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

  :global([data-theme="studio"]) input,
  :global([data-theme="studio"]) .picker-new-form .primary {
    border-radius: 0;
  }
</style>
