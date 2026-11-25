<!-- Remote (#336) and playback handoffs (#414), Settings › App, desktop only: two switches
     on the same server, then QR code, address, code and "New code" as soon as either is
     on, and "Link a phone" with handoffs (#62). Classes `.setting-row`, `.setting-switch`
     and `.update-check` taken from Settings: the themes already dress them; the QR code
     layout is the Jam's (`.jam-qr`, `.jam-fields`). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type RemoteStatus } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t, locale } from "../lib/i18n.svelte";

  let status = $state<RemoteStatus | null>(null);
  let busy = $state(false);

  const remoteOn = $derived(status?.remote ?? false);
  const handoffOn = $derived(status?.handoff ?? false);

  // Clock of the pairing window (#62): the line goes away when it closes.
  let now = $state(Date.now());
  $effect(() => {
    const timer = setInterval(() => (now = Date.now()), 10_000);
    return () => clearInterval(timer);
  });
  const pairingUntil = $derived(
    status?.pairingUntil && status.pairingUntil > now
      ? new Date(status.pairingUntil).toLocaleTimeString(locale(), { timeStyle: "short" })
      : null,
  );

  onMount(async () => {
    try {
      status = await commands.remoteStatus();
    } catch (e) {
      fail(e);
    }
  });

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  async function change(action: () => Promise<RemoteStatus | null>) {
    if (busy) return;
    busy = true;
    try {
      status = await action();
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  }

  const toggleRemote = () => change(remoteOn ? commands.stopRemote : commands.startRemote);
  const toggleHandoff = () => change(() => commands.setHandoffAccepted(!handoffOn));
  /** New relay key and 10 minutes for a phone to learn the route (#62). */
  const pair = () => change(commands.openHandoffPairing);

  async function renew() {
    try {
      status = await commands.renewRemoteCode();
      pushToast({ level: "info", message: t.remote.renewed });
    } catch (e) {
      fail(e);
    }
  }

  /** Address with the code after `#`, like the QR code: a phone opens or pastes it. */
  async function copyUrl() {
    if (!status) return;
    try {
      await navigator.clipboard.writeText(`${status.url}#${status.code}`);
      pushToast({ level: "info", message: t.remote.copied });
    } catch (e) {
      fail(e);
    }
  }

  /** `H4KX9QMT2WZP` → `H4KX-9QMT-2WZP`: easier to copy by hand (dashes are ignored). */
  const grouped = (code: string) => code.match(/.{1,4}/g)?.join("-") ?? code;
</script>

<div class="remote-panel">
  <div class="setting-row remote-toggle">
    <span class="text">
      <span class="setting-label">{t.remote.enable}</span>
      <span class="hint">{t.remote.enableHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={remoteOn}
      role="switch"
      aria-checked={remoteOn}
      aria-label={t.remote.enable}
      disabled={busy}
      onclick={toggleRemote}
    >
      <span class="knob"></span>
    </button>
  </div>

  <div class="setting-row remote-toggle handoff-toggle">
    <span class="text">
      <span class="setting-label">{t.remote.handoff}</span>
      <span class="hint">{t.remote.handoffHint}</span>
    </span>
    <button
      class="setting-switch"
      class:on={handoffOn}
      role="switch"
      aria-checked={handoffOn}
      aria-label={t.remote.handoff}
      disabled={busy}
      onclick={toggleHandoff}
    >
      <span class="knob"></span>
    </button>
  </div>

  {#if status}
    <div class="setting-row remote-info">
      {#if status.qrSvg && remoteOn}
        <!-- SVG made by the backend (qrcode crate) from the address and the code. -->
        <div class="jam-qr remote-qr" role="img" aria-label={t.remote.qr}>{@html status.qrSvg}</div>
      {/if}
      <dl class="jam-fields">
        <dt>{t.remote.address}</dt>
        <dd class="mono">{status.url}</dd>
        <dt>{t.remote.code}</dt>
        <dd class="mono remote-code">{grouped(status.code)}</dd>
      </dl>
      <div class="remote-actions">
        <button class="update-check" onclick={copyUrl}>{t.remote.copy}</button>
        <button class="update-check" onclick={renew}>{t.remote.renew}</button>
        {#if handoffOn && status.relay}
          <button class="update-check" disabled={busy} onclick={pair}>{t.remote.pair}</button>
        {/if}
      </div>
    </div>
    {#if !status.lan}
      <p class="note">{t.remote.noLan}</p>
    {:else}
      {#if remoteOn}<p class="note">{t.remote.howTo}</p>{/if}
      {#if handoffOn && status.relay}
        <p class="note">{t.remote.handoffHowTo}</p>
        {#if pairingUntil}<p class="note remote-pairing">{t.remote.pairing(pairingUntil)}</p>{/if}
        <p class="note">{t.remote.pairNote}</p>
      {:else if handoffOn}
        <p class="note">{t.remote.handoffHowToLan}</p>
      {/if}
    {/if}
  {:else}
    <p class="note">{t.remote.firewall}</p>
  {/if}
</div>

<style>
  .remote-panel {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 640px;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 12px 24px;
    padding: 12px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .setting-row.remote-toggle {
    flex-wrap: nowrap;
  }

  .setting-row.remote-info {
    gap: 16px;
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .setting-label {
    font-weight: 500;
  }

  .hint {
    font-size: 12px;
    color: var(--text-muted);
  }

  .note {
    margin: 4px 0 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .note.remote-pairing {
    color: var(--accent);
  }

  /* Switch taken from Settings: the themes already dress it by its class name. */
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

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .jam-qr {
    flex: none;
    width: 148px;
    height: 148px;
    padding: 6px;
    border-radius: 10px;
    background: #ffffff;
  }

  .jam-qr :global(svg) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .jam-fields {
    display: grid;
    grid-template-columns: auto 1fr;
    gap: 6px 16px;
    align-items: baseline;
    margin: 0;
    flex: 1;
    min-width: 220px;
  }

  .jam-fields dt {
    font-size: 13px;
    color: var(--text-muted);
  }

  .jam-fields dd {
    margin: 0;
    overflow-wrap: anywhere;
    user-select: text;
  }

  .remote-code {
    font-size: 18px;
    letter-spacing: 0.12em;
  }

  .remote-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
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
