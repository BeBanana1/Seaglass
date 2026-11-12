<!-- Télécommande (#336), Réglages › Application, bureau seulement : interrupteur, puis QR
     code, adresse, code et « Nouveau code ». Classes `.setting-row`, `.setting-switch` et
     `.update-check` reprises de Réglages : les thèmes les habillent déjà ; la mise en page
     du QR code reprend celle de la Jam (`.jam-qr`, `.jam-fields`). -->
<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type RemoteStatus } from "../lib/ipc";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let status = $state<RemoteStatus | null>(null);
  let busy = $state(false);

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

  async function toggle() {
    if (busy) return;
    busy = true;
    try {
      if (status) {
        await commands.stopRemote();
        status = null;
      } else {
        status = await commands.startRemote();
      }
    } catch (e) {
      fail(e);
    } finally {
      busy = false;
    }
  }

  async function renew() {
    try {
      status = await commands.renewRemoteCode();
      pushToast({ level: "info", message: t.remote.renewed });
    } catch (e) {
      fail(e);
    }
  }

  async function copyUrl() {
    if (!status) return;
    try {
      await navigator.clipboard.writeText(status.url);
      pushToast({ level: "info", message: t.remote.copied });
    } catch (e) {
      fail(e);
    }
  }

  /** `H4KX9QMT2WZP` → `H4KX-9QMT-2WZP` : plus facile à recopier (tirets ignorés). */
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
      class:on={status !== null}
      role="switch"
      aria-checked={status !== null}
      aria-label={t.remote.enable}
      disabled={busy}
      onclick={toggle}
    >
      <span class="knob"></span>
    </button>
  </div>

  {#if status}
    <div class="setting-row remote-info">
      {#if status.qrSvg}
        <!-- SVG produit par le backend (crate qrcode) à partir de l'adresse et du code. -->
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
      </div>
    </div>
    <p class="note">{status.lan ? t.remote.howTo : t.remote.noLan}</p>
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

  /* Interrupteur repris de Réglages : les thèmes l'habillent déjà par son nom de classe. */
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

  /* Studio : angles vifs. Atelier : angles à peine arrondis, pas de fond. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .update-check {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
    background: transparent;
  }
</style>
