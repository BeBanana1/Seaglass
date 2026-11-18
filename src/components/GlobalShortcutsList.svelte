<!-- Raccourcis globaux (#413, Réglages › Application) : une ligne par action, avec sa
     combinaison. Un clic sur la combinaison attend la suivante au clavier ; le backend
     l'enregistre auprès du système et dit celles qu'une autre app a déjà prises. -->
<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type GlobalAction } from "../lib/ipc";
  import { capture, DEFAULT_BINDINGS, displayKeys, GLOBAL_ACTIONS, sameKeys } from "../lib/hotkeys";
  import { setGlobalShortcuts, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let failed = $state<GlobalAction[]>([]);
  let recording = $state<GlobalAction | null>(null);
  let message = $state<{ action: GlobalAction; text: string } | null>(null);

  const shortcuts = $derived(ui.settings.globalShortcuts);

  onMount(refreshStatus);

  async function refreshStatus() {
    try {
      failed = (await commands.globalShortcutsStatus()).failed;
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  async function save(bindings: Partial<Record<GlobalAction, string>>) {
    try {
      await setGlobalShortcuts({ ...shortcuts, bindings });
      await refreshStatus();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function setBinding(action: GlobalAction, keys: string | null) {
    const bindings = { ...shortcuts.bindings };
    if (keys) bindings[action] = keys;
    else delete bindings[action];
    void save(bindings);
  }

  function startRecording(action: GlobalAction) {
    recording = recording === action ? null : action;
    message = null;
  }

  function onKeydown(e: KeyboardEvent, action: GlobalAction) {
    if (recording !== action) return;
    // Rien ne part aux raccourcis de l'app pendant l'enregistrement.
    e.preventDefault();
    e.stopPropagation();
    const plain = !e.ctrlKey && !e.altKey && !e.metaKey && !e.shiftKey;
    if (e.key === "Escape" && plain) {
      recording = null;
      message = null;
      return;
    }
    if ((e.key === "Backspace" || e.key === "Delete") && plain) {
      recording = null;
      message = null;
      setBinding(action, null);
      return;
    }
    const result = capture(e);
    if (result.kind === "pending") return;
    if (result.kind === "needsModifier") {
      message = { action, text: t.settings.shortcutNeedsModifier };
      return;
    }
    if (result.kind === "unsupported") {
      message = { action, text: t.settings.shortcutUnsupported };
      return;
    }
    const other = GLOBAL_ACTIONS.find(
      (a) => a !== action && shortcuts.bindings[a] && sameKeys(shortcuts.bindings[a]!, result.keys),
    );
    if (other) {
      message = { action, text: t.settings.shortcutDuplicate(t.settings.globalActions[other]) };
      return;
    }
    recording = null;
    message = null;
    setBinding(action, result.keys);
  }
</script>

<ul class="global-shortcuts">
  {#each GLOBAL_ACTIONS as action (action)}
    {@const keys = shortcuts.bindings[action]}
    <li class="global-shortcut" class:failed={failed.includes(action)}>
      <span class="global-shortcut-label">
        {t.settings.globalActions[action]}
        {#if message?.action === action}
          <span class="global-shortcut-error">{message.text}</span>
        {:else if recording === action}
          <span class="global-shortcut-note">{t.settings.shortcutRecordHint}</span>
        {:else if failed.includes(action)}
          <span class="global-shortcut-error">{t.settings.shortcutTaken}</span>
        {/if}
      </span>
      <button
        class="global-shortcut-keys"
        class:recording={recording === action}
        aria-label={t.settings.shortcutRecord(t.settings.globalActions[action])}
        onclick={() => startRecording(action)}
        onkeydown={(e) => onKeydown(e, action)}
        onblur={() => recording === action && (recording = null)}
      >
        {#if recording === action}
          {t.settings.shortcutRecording}
        {:else if keys}
          <kbd>{displayKeys(keys, t.palette.space)}</kbd>
        {:else}
          {t.settings.shortcutNone}
        {/if}
      </button>
      <button
        class="global-shortcut-clear"
        aria-label={t.settings.shortcutClear(t.settings.globalActions[action])}
        title={t.settings.shortcutClear(t.settings.globalActions[action])}
        disabled={!keys}
        onclick={() => setBinding(action, null)}
      >
        <Icon name="close" size={12} />
      </button>
    </li>
  {/each}
</ul>
<button class="global-shortcuts-reset" onclick={() => save({ ...DEFAULT_BINDINGS })}>
  {t.settings.shortcutsReset}
</button>

<style>
  .global-shortcuts {
    display: flex;
    flex-direction: column;
    gap: 6px;
    max-width: 640px;
    margin: 10px 0 0;
    padding: 0;
    list-style: none;
  }

  .global-shortcut {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 10px 8px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .global-shortcut-label {
    display: flex;
    flex: 1;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .global-shortcut-error,
  .global-shortcut-note {
    font-size: 12px;
    color: var(--text-muted);
  }

  .global-shortcut-error {
    color: var(--danger);
  }

  .global-shortcut-keys {
    min-width: 150px;
    padding: 5px 10px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    font-size: 12px;
    color: var(--text-muted);
    background: var(--bg-hover);
  }

  .global-shortcut-keys:hover,
  .global-shortcut-keys.recording {
    color: var(--text);
    border-color: var(--accent);
  }

  kbd {
    font-family: var(--font-mono);
    font-size: 11px;
    color: var(--text);
  }

  .global-shortcut-clear {
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .global-shortcut-clear:hover:not(:disabled) {
    color: var(--text);
  }

  .global-shortcut-clear:disabled {
    visibility: hidden;
  }

  .global-shortcuts-reset {
    margin-top: 10px;
    padding: 5px 12px;
    border: 1px solid var(--border);
    border-radius: 999px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .global-shortcuts-reset:hover {
    color: var(--text);
  }
</style>
