<script lang="ts">
  // "Blind test" button of the headers (#434): remembers the list and opens the game
  // page. Hidden below `BLIND_MIN_TRACKS` tracks.
  import type { TrackId } from "../lib/ipc";
  import { t } from "../lib/i18n.svelte";
  import { open } from "../lib/actions";
  import { BLIND_MIN_TRACKS, blindTest } from "../lib/stores/blindTest.svelte";
  import Icon from "./Icon.svelte";

  let { title, trackIds }: { title: string; trackIds: TrackId[] } = $props();

  function start() {
    blindTest.pool = { title, trackIds: [...trackIds] };
    open({ name: "blindTest" });
  }
</script>

{#if trackIds.length >= BLIND_MIN_TRACKS}
  <button
    class="blind-test-button"
    onclick={start}
    title={t.blindTest.start}
    aria-label={t.blindTest.start}
  >
    <Icon name="quiz" size={18} />
  </button>
{/if}

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
    clip-path: var(--control-clip);
  }

  button:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  /* Android: 44 px touch target, like the other header buttons. */
  :global(:root[data-platform="android"]) button {
    width: 44px;
    height: 44px;
  }
</style>
