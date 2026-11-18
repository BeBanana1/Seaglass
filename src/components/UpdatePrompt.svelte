<script lang="ts">
  // Mise à jour téléchargée et vérifiée par le backend : on propose, on n'impose jamais.
  // « Plus tard » laisse le backend l'installer à la fermeture de l'app. Sur Android, rien
  // n'est téléchargé d'avance (données mobiles) : « Télécharger et installer » le fait, puis
  // Android affiche sa boîte d'installation.
  import { onMount } from "svelte";
  import { commands, events, type UpdateInfo } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { num, t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";

  let update = $state<UpdateInfo | null>(null);
  let dismissed = $state(false);
  let installing = $state(false);

  onMount(() => {
    let unlisten: (() => void) | undefined;
    events
      .onUpdateReady((u) => {
        update = u;
        dismissed = false;
      })
      .then((u) => (unlisten = u));
    commands.updateStatus().then((u) => (update ??= u));
    return () => unlisten?.();
  });

  const playing = $derived(player.playing);
  const mobile = $derived(platform.mobile);
  const sizeMb = $derived(
    update?.size ? num(update.size / 1e6, { maximumFractionDigits: 0 }) : null,
  );

  async function install() {
    installing = true;
    try {
      await commands.installUpdate();
      // Android : la boîte d'installation du système est ouverte, l'app sera remplacée.
      if (mobile) installing = false;
    } catch (e) {
      installing = false;
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

{#if update && !dismissed}
  <div class="prompt" class:mobile role="alertdialog" aria-labelledby="update-title">
    <span class="label">{mobile ? t.settings.update.available : t.settings.update.ready}</span>
    <h3 id="update-title" class="display">Seaglass {update.version}</h3>
    <p class="sub">{t.settings.update.current(update.currentVersion)}</p>
    {#if update.notes}<pre class="notes">{update.notes}</pre>{/if}
    {#if playing}<p class="sub">{t.settings.update.playbackStops}</p>{/if}
    <div class="actions">
      <button class="primary" onclick={install} disabled={installing}>
        {#if mobile}
          {installing ? t.settings.update.downloading : t.settings.update.download(sizeMb)}
        {:else}
          {installing ? t.settings.update.installing : t.settings.update.install}
        {/if}
      </button>
      <button
        class="later"
        onclick={() => (dismissed = true)}
        title={mobile ? undefined : t.settings.update.laterHint}
      >
        {t.settings.update.later}
      </button>
    </div>
  </div>
{/if}

<style>
  .prompt {
    position: fixed;
    left: 20px;
    bottom: calc(var(--player-height) + 24px);
    z-index: 55;
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: min(360px, calc(100vw - 40px));
    padding: 18px;
    border: var(--dialog-border);
    border-radius: var(--dialog-radius);
    color: var(--text);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--dialog-shadow);
  }

  /* Au-dessus du mini-lecteur et de la navigation du bas. */
  .prompt.mobile {
    right: 12px;
    left: 12px;
    bottom: calc(var(--safe-bottom) + 136px);
    width: auto;
  }

  h3 {
    margin: 0;
    font-size: 20px;
  }

  .sub {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
  }

  .notes {
    max-height: 140px;
    margin: 4px 0;
    overflow: auto;
    font: inherit;
    font-size: 13px;
    white-space: pre-wrap;
  }

  .actions {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  .actions button {
    padding: 7px 14px;
    border-radius: 999px;
    font-weight: 600;
  }

  .primary {
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .later {
    border: 1px solid var(--border) !important;
  }

  .later:hover {
    background: var(--bg-hover);
  }
</style>
