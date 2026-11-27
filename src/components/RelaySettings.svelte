<!-- Relay for online Jam and transfers (#452): the one compiled in by default, another
     (address checked by the backend), or none. The key is never read back: only
     "a key is saved". Classes `.setting-row`, `.setting-switch` and `.update-check`
     taken from Settings: the themes already style them. -->
<script lang="ts">
  import { commands, type RelayChange, type RelayView } from "../lib/ipc";
  import { refreshJamOnline } from "../lib/stores/jam.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let relay = $state<RelayView | null>(null);
  let address = $state("");
  let key = $state("");
  let busy = $state(false);

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  function show(view: RelayView) {
    relay = view;
    address = view.url ?? view.defaultUrl ?? "";
    key = "";
  }

  $effect(() => {
    commands.relaySettings().then(show, fail);
  });

  async function apply(change: RelayChange, toast: boolean) {
    if (busy) return;
    busy = true;
    try {
      show(await commands.setRelaySettings(change));
      await refreshJamOnline();
      if (toast) pushToast({ level: "info", message: t.jam.relay.saved });
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  }

  function toggle() {
    if (!relay) return;
    void apply({ enabled: !relay.enabled, url: relay.url }, false);
  }

  function save() {
    if (!relay) return;
    // Empty key: unchanged (`null` on the Rust side keeps the saved one).
    void apply({ enabled: true, url: address, key: key.trim() ? key : null }, true);
  }

  function reset() {
    if (!relay) return;
    void apply({ enabled: true, url: null, key: "" }, true);
  }

  const changed = $derived(
    !!relay &&
      (address.trim().replace(/\/+$/, "") !== (relay.url ?? relay.defaultUrl ?? "") ||
        key.trim() !== ""),
  );
</script>

{#if relay}
  <div class="relay-settings">
    <label class="setting-row">
      <span class="text">
        <span class="setting-label">{t.jam.relay.enable}</span>
        <span class="hint">{t.jam.relay.enableHint}</span>
      </span>
      <button
        class="setting-switch"
        class:on={relay.enabled}
        role="switch"
        aria-checked={relay.enabled}
        aria-label={t.jam.relay.enable}
        disabled={busy}
        onclick={toggle}
      >
        <span class="knob"></span>
      </button>
    </label>
    {#if relay.enabled}
      <div class="setting-row relay-form">
        <label class="field">
          <span class="setting-label">{t.jam.relay.address}</span>
          <input
            class="mono"
            type="url"
            spellcheck="false"
            autocomplete="off"
            placeholder={relay.defaultUrl ?? "https://…"}
            bind:value={address}
          />
        </label>
        <label class="field">
          <span class="setting-label">{t.jam.relay.key}</span>
          <input
            class="mono"
            type="text"
            spellcheck="false"
            autocomplete="off"
            placeholder={relay.customKey ? t.jam.relay.keyKept : t.jam.relay.keyBuild}
            bind:value={key}
          />
          <span class="hint">{t.jam.relay.keyHint}</span>
        </label>
        <p class="hint status" class:warn={!relay.usable} role="status">
          {relay.usable ? t.jam.relay.usable : t.jam.relay.missing}
        </p>
        <div class="actions">
          <button class="update-check" onclick={save} disabled={busy || !changed}>
            {busy ? t.jam.relay.saving : t.jam.relay.save}
          </button>
          {#if relay.url !== null || relay.customKey}
            <button class="update-check" onclick={reset} disabled={busy}>
              {relay.url !== null ? t.jam.relay.reset : t.jam.relay.resetKey}
            </button>
          {/if}
        </div>
      </div>
    {/if}
  </div>
{/if}

<style>
  .relay-settings {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 24px;
    max-width: 640px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .setting-row.relay-form {
    flex-direction: column;
    align-items: stretch;
    gap: 12px;
  }

  .text,
  .field {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
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
    gap: 8px;
  }

  /* Switch taken from Settings: the themes already style it through its class name. */
  .setting-switch {
    position: relative;
    flex-shrink: 0;
    width: 40px;
    height: 22px;
    border-radius: 999px;
    background: var(--bg-hover);
    box-shadow: inset 0 0 0 1px var(--border);
    transition: background var(--motion-fast);
  }

  .setting-switch .knob {
    position: absolute;
    top: 3px;
    left: 3px;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    background: var(--text-muted);
    transition:
      transform var(--motion-fast),
      background var(--motion-fast);
  }

  .setting-switch.on {
    background: var(--accent);
  }

  .setting-switch.on .knob {
    transform: translateX(18px);
    background: var(--accent-contrast);
  }

  .setting-switch:disabled {
    opacity: 0.6;
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

  /* Studio: sharp corners. Atelier: barely rounded corners, no background. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .update-check {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
    background: transparent;
  }
</style>
