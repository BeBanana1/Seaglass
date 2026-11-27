<!-- Sending listens to ListenBrainz (#431): paste the token (verified by the backend), see
     the connected account and the pending listens, remove. The token is never read back by
     the UI. `.setting-row` and `.update-check` classes reused from Settings: the themes
     already style them. -->
<script lang="ts">
  import { commands, type ListenBrainzStatus } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let status = $state<ListenBrainzStatus | null>(null);
  let token = $state("");
  let busy = $state(false);

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  $effect(() => {
    commands.listenbrainzStatus().then((s) => (status = s), fail);
  });

  async function run(action: () => Promise<ListenBrainzStatus>, done?: string) {
    if (busy) return;
    busy = true;
    try {
      status = await action();
      token = "";
      if (done) pushToast({ level: "info", message: done });
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  }

  const connect = () =>
    run(() => commands.listenbrainzConnect(token), t.settings.listenbrainz.done);
  const disconnect = () => run(() => commands.listenbrainzDisconnect());
  // An account whose token was refused keeps the field, to paste a new one.
  const askToken = $derived(!!status && (status.user === null || status.refused));
</script>

{#if status}
  <div class="setting-row listenbrainz">
    {#if status.user !== null}
      <p class="hint status" class:warn={status.refused} role="status">
        {status.refused
          ? t.settings.listenbrainz.refused
          : t.settings.listenbrainz.connected(status.user)}
      </p>
      {#if status.pending > 0}
        <p class="hint">{t.settings.listenbrainz.pending(status.pending)}</p>
      {/if}
    {/if}
    {#if askToken}
      <form class="field" onsubmit={(e) => (e.preventDefault(), connect())}>
        <label class="field">
          <span class="setting-label">{t.settings.listenbrainz.token}</span>
          <input
            class="mono"
            type="password"
            spellcheck="false"
            autocomplete="off"
            bind:value={token}
          />
          <span class="hint">{t.settings.listenbrainz.tokenHint}</span>
        </label>
        <div class="actions">
          <button class="update-check" type="submit" disabled={busy || !token.trim()}>
            {busy ? t.settings.listenbrainz.connecting : t.settings.listenbrainz.connect}
          </button>
        </div>
      </form>
    {/if}
    {#if status.user !== null}
      <div class="actions">
        <button class="update-check" onclick={disconnect} disabled={busy}>
          {t.settings.listenbrainz.disconnect}
        </button>
        <span class="hint">{t.settings.listenbrainz.disconnectHint}</span>
      </div>
    {/if}
  </div>
{/if}

<style>
  .setting-row {
    display: flex;
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
    max-width: 640px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  form.field {
    gap: 10px;
  }

  .setting-label {
    font-weight: 500;
  }

  .hint {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .status {
    color: var(--accent);
  }

  .status.warn {
    color: var(--warn, var(--text-muted));
  }

  input {
    width: 100%;
    padding: 8px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    color: var(--text);
    background: var(--bg-hover);
  }

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px 12px;
  }

  .update-check {
    flex-shrink: 0;
    padding: 6px 14px;
    border: 1px solid var(--border) !important;
    border-radius: 999px;
    font-weight: 600;
    background: var(--control-bg, none);
  }

  .update-check:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .update-check:disabled {
    opacity: 0.6;
    cursor: default;
  }

  /* Studio: sharp corners. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .update-check {
    border-radius: 0;
  }
</style>
