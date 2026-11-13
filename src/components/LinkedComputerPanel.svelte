<!-- Ordinateur lié (#414), Réglages › Application, Android : lier en tapant l'adresse et le
     code de la télécommande du PC, puis « Reprendre ici » / « Continuer sur l'ordinateur »
     (aussi dans « En lecture »). Classes `.setting-row` et `.update-check` reprises de
     Réglages : les thèmes les habillent déjà ; propres : `.handoff-link`, `.handoff-input`,
     `.handoff-actions`. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { commands } from "../lib/ipc";
  import { handoff, loadLinkedComputer, pullHere, pushToComputer } from "../lib/stores/handoff.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let text = $state("");
  let linking = $state(false);

  onMount(loadLinkedComputer);

  async function link(e: SubmitEvent) {
    e.preventDefault();
    if (linking || !text.trim()) return;
    linking = true;
    try {
      handoff.linked = await commands.linkComputer(text);
      text = "";
      pushToast({ level: "info", message: t.handoff.linked });
    } catch (err) {
      pushToast({ level: "error", message: String(err) });
    } finally {
      linking = false;
    }
  }

  async function unlink() {
    try {
      await commands.unlinkComputer();
      handoff.linked = null;
      pushToast({ level: "info", message: t.handoff.unlinked });
    } catch (err) {
      pushToast({ level: "error", message: String(err) });
    }
  }
</script>

<div class="handoff-panel">
  {#if handoff.linked}
    <div class="setting-row handoff-link">
      <span class="text">
        <span class="setting-label">{t.handoff.linkedTo(handoff.linked.host)}</span>
      </span>
      <button class="update-check" onclick={unlink}>{t.handoff.unlink}</button>
    </div>
    <div class="setting-row handoff-actions">
      <span class="text">
        <span class="setting-label">{t.handoff.pull}</span>
        <span class="hint">{t.handoff.pullHint}</span>
      </span>
      <button class="update-check" disabled={handoff.busy} onclick={pullHere}>{t.handoff.pull}</button>
    </div>
    <div class="setting-row handoff-actions">
      <span class="text">
        <span class="setting-label">{t.handoff.push}</span>
        <span class="hint">{t.handoff.pushHint}</span>
      </span>
      <button class="update-check" disabled={handoff.busy} onclick={pushToComputer}>{t.handoff.push}</button>
    </div>
  {:else}
    <form class="setting-row handoff-link" onsubmit={link}>
      <label class="text">
        <span class="setting-label">{t.handoff.linkLabel}</span>
        <input
          class="handoff-input"
          type="text"
          inputmode="url"
          autocomplete="off"
          autocapitalize="characters"
          spellcheck="false"
          placeholder={t.handoff.linkPlaceholder}
          bind:value={text}
        />
        <span class="hint">{t.handoff.linkHint}</span>
      </label>
      <button class="update-check" type="submit" disabled={linking || !text.trim()}>{t.handoff.link}</button>
    </form>
  {/if}
</div>

<style>
  .handoff-panel {
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

  .text {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
    flex: 1;
  }

  .setting-label {
    font-weight: 500;
  }

  .hint {
    font-size: 12px;
    color: var(--text-muted);
  }

  .handoff-input {
    width: 100%;
    padding: 8px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
    color: var(--text);
    font-family: var(--font-mono);
    font-size: 13px;
  }

  .handoff-input:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
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
  }

  /* Studio : angles vifs. Atelier : angles à peine arrondis, pas de fond. */
  :global([data-theme="studio"]) .setting-row,
  :global([data-theme="studio"]) .update-check,
  :global([data-theme="studio"]) .handoff-input {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .setting-row {
    border-radius: 2px;
    background: transparent;
  }
</style>
