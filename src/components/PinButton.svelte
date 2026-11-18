<script lang="ts">
  import type { Pin } from "../lib/ipc";
  import { isPinned, MAX_DRIVE_TILES, togglePin } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { platform } from "../lib/platform.svelte";
  import { t } from "../lib/i18n.svelte";
  import { pinLabel } from "../lib/pins";
  import Icon from "./Icon.svelte";

  // Bureau : barre latérale. Android : tuiles du mode conduite (#193), 8 au plus.
  let { pin }: { pin: Pin } = $props();

  const pinned = $derived(isPinned(pin));
  const label = $derived(
    platform.mobile ? (pinned ? t.drive.unpin : t.drive.pin) : pinned ? t.menu.unpin : t.menu.pin,
  );

  async function toggle() {
    try {
      if (!platform.mobile) {
        await togglePin(pin);
        return;
      }
      const adding = !pinned;
      if (!(await togglePin(pin, MAX_DRIVE_TILES))) {
        pushToast({ level: "error", message: t.drive.full(MAX_DRIVE_TILES) });
      } else if (adding) {
        pushToast({ level: "info", message: t.drive.pinned(pinLabel(pin)) });
      }
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<button
  class="pin-button"
  class:pinned
  onclick={toggle}
  title={label}
  aria-label={label}
  aria-pressed={pinned}
>
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
