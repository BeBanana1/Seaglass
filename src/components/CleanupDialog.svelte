<script lang="ts">
  // « Nettoyer » une liste (#382) : votre playlist ou vos titres likés. Le backend propose
  // les changements (doublons, titres indisponibles et leurs remplaçants) avec l'opération de
  // chacun ; on coche, on applique, et un toast propose « Annuler ».
  import { onMount } from "svelte";
  import type { CleanupItem, CleanupReport, ListOp, ListOpsResult } from "../lib/ipc";
  import { artistNames, fullTitle } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { pushToast, pushUndoToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";

  let {
    title,
    positions,
    load,
    apply,
    undo,
    onClose,
    onChanged,
  }: {
    title: string;
    /** Afficher les positions (playlist) ; pas pour les favoris. */
    positions: boolean;
    load: () => Promise<CleanupReport>;
    apply: (ops: ListOp[]) => Promise<ListOpsResult>;
    undo: (ops: ListOp[]) => Promise<ListOpsResult>;
    onClose: () => void;
    /** La liste a (peut-être) changé : à relire. */
    onChanged: () => void;
  } = $props();

  const c = t.views.cleanup;
  // Jamais modifié : `raw`, ses opérations repartent en objets simples.
  let report = $state.raw<CleanupReport | null>(null);
  let error = $state<string | null>(null);
  // Cases cochées, par position dans la liste lue.
  let chosen = $state<Record<number, boolean>>({});
  let busy = $state(false);
  let dialog = $state<HTMLElement>();

  onMount(() => {
    requestAnimationFrame(() => dialog?.focus());
    load()
      .then((r) => {
        report = r;
        chosen = Object.fromEntries(r.items.map((i) => [i.index, i.suggested]));
      })
      .catch((e) => (error = String(e)));
  });

  const duplicates = $derived(report?.items.filter((i) => i.kind === "duplicate") ?? []);
  const unavailable = $derived(report?.items.filter((i) => i.kind === "unavailable") ?? []);
  const count = $derived(report?.items.filter((i) => chosen[i.index]).length ?? 0);

  async function run() {
    if (!report || busy || count === 0) return;
    const ops = report.items.filter((i) => chosen[i.index]).map((i) => i.op);
    busy = true;
    try {
      const result = await apply(ops);
      if (result.error) pushToast({ level: "error", message: c.partial(result.applied, result.error) });
      if (result.applied > 0) pushUndoToast(c.done(result.applied), () => void revert(result.undo));
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    // Relue même en cas d'échec : un refus signifie que la liste a changé.
    onChanged();
    onClose();
  }

  // Le dialogue est déjà fermé : `undo` et `onChanged` viennent du parent.
  async function revert(ops: ListOp[]) {
    try {
      const result = await undo(ops);
      if (result.error) pushToast({ level: "error", message: c.partial(result.applied, result.error) });
      else pushToast({ level: "info", message: c.undone(result.applied, result.skipped) });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    onChanged();
  }

  function detail(item: CleanupItem): string {
    if (item.kind === "duplicate") {
      const text = item.sameTrack ? c.sameTrack : c.edition(fullTitle(item.kept));
      return positions ? `${text} (${c.position(item.keptIndex + 1)})` : text;
    }
    return item.replacement
      ? `${c.replaceWith} : ${fullTitle(item.replacement)} · ${item.replacement.album.title}`
      : c.noReplacement;
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key !== "Escape" || busy) return;
    event.stopPropagation();
    onClose();
  }
</script>

{#snippet rows(items: CleanupItem[])}
  <ul>
    {#each items as item (item.index)}
      <li>
        <label class="cleanup-row" class:lost={item.kind === "unavailable" && !item.replacement}>
          <input type="checkbox" bind:checked={chosen[item.index]} disabled={busy} />
          <span class="cleanup-text">
            <span class="cleanup-title">
              {#if positions}<span class="cleanup-position">{c.position(item.index + 1)}</span>{/if}
              {fullTitle(item.track)}
            </span>
            <span class="cleanup-sub">{artistNames(item.track)} · {item.track.album.title}</span>
            <span class="cleanup-detail">{detail(item)}</span>
          </span>
        </label>
      </li>
    {/each}
  </ul>
{/snippet}

<!-- svelte-ignore a11y_no_static_element_interactions, a11y_click_events_have_key_events -->
<div class="cleanup-backdrop" onclick={(event) => event.target === event.currentTarget && !busy && onClose()}>
  <div
    class="cleanup-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="cleanup-title"
    tabindex="-1"
    onkeydown={onKeydown}
    bind:this={dialog}
  >
    <header>
      <h3 id="cleanup-title" class="display">{title}</h3>
      <button class="close" onclick={onClose} disabled={busy} aria-label={t.common.close}><Icon name="close" /></button>
    </header>

    {#if error}
      <p class="error">{error}</p>
    {:else if !report}
      <p class="muted">{c.scanning}</p>
    {:else if report.items.length === 0}
      <p class="muted">{c.nothing(report.scanned)}</p>
    {:else}
      <div class="cleanup-list">
        {#if duplicates.length > 0}
          <h4>{c.duplicates(duplicates.length)}</h4>
          {@render rows(duplicates)}
        {/if}
        {#if unavailable.length > 0}
          <h4>{c.unavailable(unavailable.length)}</h4>
          {@render rows(unavailable)}
          <p class="muted note">{c.irreversible}</p>
        {/if}
      </div>
    {/if}

    <div class="buttons">
      <button class="cancel" onclick={onClose} disabled={busy}>{t.common.cancel}</button>
      {#if report && report.items.length > 0}
        <button class="primary" onclick={run} disabled={busy || count === 0}>
          {busy ? c.applying : c.apply(count)}
        </button>
      {/if}
    </div>
  </div>
</div>

<style>
  .cleanup-backdrop {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: grid;
    place-items: center;
    padding: 24px;
    background: rgb(0 0 0 / 0.45);
  }

  .cleanup-dialog {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(560px, 100%);
    max-height: min(720px, 100%);
    padding: 18px;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
  }

  .cleanup-dialog:focus {
    outline: none;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
  }

  h3 {
    margin: 0;
    font-size: 20px;
  }

  h4 {
    margin: 4px 0 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .close {
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .close:hover:not(:disabled) {
    color: var(--text);
  }

  .muted {
    margin: 0;
    color: var(--text-muted);
    line-height: 1.5;
  }

  .note {
    margin-top: 8px;
    font-size: 12px;
  }

  .error {
    margin: 0;
    color: var(--danger);
  }

  .cleanup-list {
    overflow-y: auto;
    min-height: 0;
  }

  ul {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .cleanup-row {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px;
    border-radius: var(--radius);
    cursor: pointer;
  }

  .cleanup-row:hover {
    background: var(--bg-hover);
  }

  .cleanup-row input {
    margin-top: 3px;
    accent-color: var(--accent);
  }

  .cleanup-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .cleanup-title {
    font-weight: 600;
  }

  .cleanup-position {
    margin-right: 6px;
    font-weight: 400;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .cleanup-sub,
  .cleanup-detail {
    font-size: 13px;
    color: var(--text-muted);
  }

  .cleanup-detail {
    color: var(--accent);
  }

  .lost .cleanup-detail {
    color: var(--danger);
  }

  .buttons {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
  }

  .buttons button {
    padding: 8px 16px;
    border-radius: var(--radius-pill);
    font-weight: 600;
  }

  .buttons button:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .cancel {
    border: 1px solid var(--border);
    color: var(--text);
  }

  .cancel:hover:not(:disabled) {
    background: var(--bg-hover);
  }

  .primary {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  :global([data-theme="studio"]) .cleanup-dialog .buttons button {
    border-radius: 0;
  }
</style>
