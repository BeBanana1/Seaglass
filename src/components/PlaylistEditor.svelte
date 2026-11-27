<script lang="ts">
  // Edit one of your playlists (#201): title, description, or deletion after
  // confirmation. The parent applies the result (`onSaved`, `onDeleted`).
  import { commands, type PlaylistSummary } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";

  let {
    playlist,
    onClose,
    onSaved,
    onDeleted,
    onCleanup,
  }: {
    playlist: PlaylistSummary;
    onClose: () => void;
    onSaved: (title: string, description: string) => void;
    onDeleted: () => void;
    /** Opens "Clean up" (#382) in place of this dialog. */
    onCleanup: () => void;
  } = $props();

  const e = t.views.playlistEdit;
  // Starting values copied once: the dialog is recreated on every opening.
  // svelte-ignore state_referenced_locally
  let title = $state(playlist.title);
  // svelte-ignore state_referenced_locally
  let description = $state(playlist.description ?? "");
  let confirming = $state(false);
  let busy = $state(false);
  let titleInput = $state<HTMLInputElement>();
  let dialog = $state<HTMLElement>();

  $effect(() => {
    requestAnimationFrame(() => titleInput?.focus());
  });

  $effect(() => {
    // The confirmation takes focus so that Enter does not delete by mistake.
    if (confirming)
      requestAnimationFrame(() => dialog?.querySelector<HTMLElement>(".cancel")?.focus());
  });

  async function save(event: SubmitEvent) {
    event.preventDefault();
    const nextTitle = title.trim();
    const nextDescription = description.trim();
    if (!nextTitle || busy) return;
    busy = true;
    try {
      await commands.editPlaylist(playlist.uuid, nextTitle, nextDescription);
      onSaved(nextTitle, nextDescription);
    } catch (err) {
      pushToast({ level: "error", message: String(err) });
    } finally {
      busy = false;
    }
  }

  async function remove() {
    if (busy) return;
    busy = true;
    try {
      await commands.deletePlaylist(playlist.uuid);
      pushToast({ level: "info", message: t.views.playlistEdit.deleted(playlist.title) });
      onDeleted();
    } catch (err) {
      pushToast({ level: "error", message: String(err) });
      busy = false;
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape") return;
    event.stopPropagation();
    if (confirming) confirming = false;
    else onClose();
  }
</script>

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="editor-backdrop" onclick={(event) => event.target === event.currentTarget && onClose()}>
  <div
    class="playlist-editor"
    role="dialog"
    aria-modal="true"
    aria-labelledby="playlist-editor-title"
    tabindex="-1"
    onkeydown={onKeydown}
    bind:this={dialog}
  >
    <header>
      <h3 id="playlist-editor-title" class="display">{confirming ? e.deleteTitle : e.title}</h3>
      <button class="close" onclick={onClose} aria-label={t.common.close}>
        <Icon name="close" />
      </button>
    </header>

    {#if confirming}
      <p class="confirm-text">{e.deleteConfirm(playlist.title)}</p>
      <div class="buttons">
        <button class="cancel" onclick={() => (confirming = false)} disabled={busy}>
          {e.cancel}
        </button>
        <button class="danger" onclick={remove} disabled={busy}>{e.delete}</button>
      </div>
    {:else}
      <form onsubmit={save}>
        <label>
          <span>{e.name}</span>
          <input type="text" maxlength="200" bind:value={title} bind:this={titleInput} required />
        </label>
        <label>
          <span>{e.description}</span>
          <textarea rows="4" maxlength="2000" bind:value={description}></textarea>
        </label>
        <button type="button" class="cleanup-link" onclick={onCleanup} disabled={busy}>
          {e.cleanupAction}
        </button>
        <div class="buttons">
          <button
            type="button"
            class="delete-link"
            onclick={() => (confirming = true)}
            disabled={busy}
          >
            {e.deleteAction}
          </button>
          <span class="spacer"></span>
          <button type="button" class="cancel" onclick={onClose} disabled={busy}>{e.cancel}</button>
          <button type="submit" class="primary" disabled={busy || !title.trim()}>{e.save}</button>
        </div>
      </form>
    {/if}
  </div>
</div>

<style>
  .editor-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.45);
  }

  .playlist-editor {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(480px, 100%);
    padding: 18px;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
  }

  .playlist-editor:focus {
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

  .close {
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .close:hover {
    color: var(--text);
  }

  form {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
    font-size: 13px;
    color: var(--text-muted);
  }

  input,
  textarea {
    width: 100%;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-raised);
    font: inherit;
    resize: vertical;
  }

  .confirm-text {
    margin: 0;
    line-height: 1.5;
  }

  .buttons {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .spacer {
    flex: 1;
  }

  .buttons button {
    padding: 8px 16px;
    border-radius: var(--radius-pill);
    font-weight: 600;
  }

  .buttons button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .cancel {
    border: 1px solid var(--border);
    color: var(--text);
  }

  .cancel:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .primary {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .danger {
    color: #fff;
    background: var(--danger);
  }

  .confirm-text + .buttons {
    justify-content: flex-end;
  }

  .delete-link {
    padding-left: 0 !important;
    padding-right: 0 !important;
    font-weight: 500 !important;
    color: var(--danger);
  }

  .delete-link:hover:not(:disabled),
  .cleanup-link:hover:not(:disabled) {
    text-decoration: underline;
  }

  .cleanup-link {
    align-self: flex-start;
    padding: 0;
    font-weight: 500;
    color: var(--accent);
  }

  :global([data-theme="studio"]) input,
  :global([data-theme="studio"]) textarea,
  :global([data-theme="studio"]) .buttons button {
    border-radius: 0;
  }
</style>
