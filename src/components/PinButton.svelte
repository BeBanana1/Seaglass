<script lang="ts">
  import type { Pin } from "../lib/ipc";
  import { isPinned, togglePin } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  let { pin }: { pin: Pin } = $props();

  const pinned = $derived(isPinned(pin));
  const label = $derived(pinned ? t.menu.unpin : t.menu.pin);

  async function toggle() {
    try {
      await togglePin(pin);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<button class="pin-button" class:pinned onclick={toggle} title={label} aria-label={label} aria-pressed={pinned}>
  <Icon name="pin" size={18} />
</button>

<style>
  button {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 37px;
    height: 37px;
    border: 1px solid var(--border);
    border-radius: var(--radius-pill);
    color: var(--text-muted);
  }

  button:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  button.pinned {
    color: var(--accent);
  }
</style>
