<!-- Jam content (#25): start, connection state, QR code, address, code, stop.
     Shared by Settings › Application and the dialog opened from "Now playing"
     (JamDialog), with the host controls (JamHostControls, #288). Classes `.setting-row` and
     `.update-check` come from Settings: the themes already dress them. -->
<script lang="ts">
  import type { JamMode } from "../lib/ipc";
  import JamHostControls from "./JamHostControls.svelte";
  import { jam, startJam, stopJam } from "../lib/stores/jam.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { platform } from "../lib/platform.svelte";
  import { t } from "../lib/i18n.svelte";

  // The local network only exists on desktop: a phone opens no port.
  const desktop = $derived(!platform.mobile);

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  async function launch(mode: JamMode) {
    try {
      await startJam(mode);
    } catch (e) {
      fail(e);
    }
  }

  async function end() {
    try {
      await stopJam();
      pushToast({ level: "info", message: t.jam.stopped });
    } catch (e) {
      fail(e);
    }
  }

  async function copyUrl() {
    if (!jam.status?.url) return;
    try {
      await navigator.clipboard.writeText(jam.status.url);
      pushToast({ level: "info", message: t.jam.copied });
    } catch (e) {
      fail(e);
    }
  }
</script>

<div class="jam-panel">
  {#if jam.status}
    <p
      class="jam-connection"
      class:warn={jam.status.connection === "reconnecting" || jam.status.connection === "failed"}
      role="status"
    >
      {jam.status.mode === "local"
        ? t.jam.connection.local
        : t.jam.connection[jam.status.connection]}
      {#if jam.status.reason}<span class="jam-reason">({jam.status.reason})</span>{/if}
    </p>
    <div class="setting-row jam-info">
      {#if jam.status.qrSvg}
        <!-- SVG made by the backend (qrcode crate) from the address and the code. -->
        <div class="jam-qr" role="img" aria-label={t.jam.qr}>{@html jam.status.qrSvg}</div>
      {/if}
      <dl class="jam-fields">
        <dt>{t.jam.address}</dt>
        <dd class="mono jam-url">{jam.status.url ?? "…"}</dd>
        <dt>{t.jam.code}</dt>
        <dd class="mono jam-code">{jam.status.code}</dd>
      </dl>
      <div class="jam-actions">
        <button class="update-check" onclick={copyUrl} disabled={!jam.status.url}>
          {t.jam.copy}
        </button>
        <button class="update-check" onclick={end}>{t.jam.stop}</button>
      </div>
    </div>
    <JamHostControls status={jam.status} />
    <p class="note">{jam.status.lan ? t.jam.howTo : t.jam.noLan}</p>
  {:else}
    <!-- Online by default (relay, no shared Wi-Fi); the local network as a second choice. -->
    <div class="setting-row jam-start">
      <span class="text">
        <span class="hint">{jam.onlineAvailable ? t.jam.onlineHint : t.jam.localHint}</span>
      </span>
      <div class="jam-actions">
        {#if jam.onlineAvailable}
          <button class="update-check" onclick={() => launch("online")} disabled={jam.busy}>
            {jam.busy ? t.jam.starting : t.jam.start}
          </button>
          {#if desktop}
            <button class="update-check" onclick={() => launch("local")} disabled={jam.busy}>
              {t.jam.startLocal}
            </button>
          {/if}
        {:else}
          <button class="update-check" onclick={() => launch("local")} disabled={jam.busy}>
            {jam.busy ? t.jam.starting : t.jam.start}
          </button>
        {/if}
      </div>
    </div>
    {#if desktop}
      <p class="note">{t.jam.localHint} {t.jam.firewall}</p>
    {:else}
      <p class="note">{t.jam.phoneHint}</p>
    {/if}
  {/if}
  <!-- TIDAL's terms (#39): personal use, no public playback. -->
  <p class="note jam-private">{t.jam.privateUse}</p>
</div>

<style>
  .hint {
    max-width: 640px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .note {
    margin: 12px 0 0;
    font-size: 12px;
    color: var(--text-muted);
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

  .setting-row .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .setting-row .hint {
    margin: 0;
    font-size: 12px;
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

  .mono {
    font-family: var(--font-mono);
    font-size: 12px;
  }

  .jam-connection {
    margin: 0 0 10px;
    font-size: 13px;
    font-weight: 600;
    color: var(--accent);
  }

  .jam-connection.warn {
    color: var(--warn, var(--text-muted));
  }

  .jam-reason {
    font-weight: 400;
    color: var(--text-muted);
  }

  .setting-row.jam-info {
    gap: 16px;
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

  .jam-code {
    font-size: 22px;
    letter-spacing: 0.2em;
  }

  .jam-actions {
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
