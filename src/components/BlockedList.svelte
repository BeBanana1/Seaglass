<!-- Titres et artistes bloqués (#334), dans Réglages › Lecture : lus chez le service à la
     demande (pas à chaque ouverture des réglages), avec « Débloquer ». Classes `.setting-row`
     et `.update-check` reprises de Réglages : les thèmes les habillent déjà. -->
<script lang="ts">
  import { commands, type BlockTarget, type Blocked } from "../lib/ipc";
  import { fullTitle } from "../lib/format";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { t } from "../lib/i18n.svelte";

  let list = $state<Blocked | null>(null);
  let loading = $state(false);
  /** Déblocages en cours, par `kind:id`. */
  let pending = $state(new Set<string>());

  const empty = $derived(list !== null && list.tracks.length === 0 && list.artists.length === 0);

  function fail(e: unknown) {
    pushToast({ level: "error", message: String(e) });
  }

  async function load() {
    loading = true;
    try {
      list = await commands.blocked();
    } catch (e) {
      fail(e);
    } finally {
      loading = false;
    }
  }

  async function unblock(target: BlockTarget) {
    const key = `${target.kind}:${target.id}`;
    pending = new Set(pending).add(key);
    try {
      await commands.setBlocked(target, false);
      if (list) {
        list =
          target.kind === "track"
            ? { ...list, tracks: list.tracks.filter((tr) => tr.id !== target.id) }
            : { ...list, artists: list.artists.filter((a) => a.id !== target.id) };
      }
    } catch (e) {
      fail(e);
    } finally {
      const next = new Set(pending);
      next.delete(key);
      pending = next;
    }
  }
</script>

{#if list === null}
  <button class="update-check" onclick={load} disabled={loading}>{t.settings.showBlocked}</button>
{:else if empty}
  <p class="hint">{t.settings.blockedEmpty}</p>
{:else}
  {#if list.tracks.length > 0}
    <h3>{t.settings.blockedTracks}</h3>
    <ul class="blocked-list">
      {#each list.tracks as track (track.id)}
        <li class="setting-row">
          <span class="text">
            <span class="setting-label">{fullTitle(track)}</span>
            <span class="hint">{track.artists.map((a) => a.name).join(", ")}</span>
          </span>
          <button
            class="update-check"
            disabled={pending.has(`track:${track.id}`)}
            onclick={() => unblock({ kind: "track", id: track.id })}
          >
            {t.settings.unblock}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
  {#if list.artists.length > 0}
    <h3>{t.settings.blockedArtists}</h3>
    <ul class="blocked-list">
      {#each list.artists as artist (artist.id)}
        <li class="setting-row">
          <span class="text"><span class="setting-label">{artist.name}</span></span>
          <button
            class="update-check"
            disabled={pending.has(`artist:${artist.id}`)}
            onclick={() => unblock({ kind: "artist", id: artist.id })}
          >
            {t.settings.unblock}
          </button>
        </li>
      {/each}
    </ul>
  {/if}
{/if}

<style>
  h3 {
    margin: 14px 0 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-muted);
  }

  .hint {
    margin: 0;
    max-width: 640px;
    font-size: 13px;
    color: var(--text-muted);
  }

  .blocked-list {
    display: flex;
    flex-direction: column;
    gap: 6px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .setting-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    max-width: 640px;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .setting-row .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .setting-row .setting-label {
    overflow: hidden;
    font-weight: 500;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .setting-row .hint {
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
</style>
