<!-- Bandeau persistant quand mpv ne tourne pas (#92). Lu dans `player-state` plutôt que
     reçu en toast : un échec au lancement précède l'écoute des événements par la webview.
     Pas de bouton de fermeture : tant que mpv ne tourne pas, rien ne peut être lu. -->
<script lang="ts">
  import { player } from "../lib/stores/player.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon from "./Icon.svelte";

  const issue = $derived(player.state.mpvIssue);
</script>

{#if issue}
  <div class="mpv-issue" role="alert">
    <span class="mpv-issue-icon"><Icon name="info" size={18} /></span>
    <span class="text">
      <span class="mpv-issue-title">{issue === "cpuUnsupported" ? t.player.mpvCpu : t.player.mpvStopped}</span>
      <span class="mpv-issue-hint">{issue === "cpuUnsupported" ? t.player.mpvCpuHint : t.player.mpvStoppedHint}</span>
    </span>
  </div>
{/if}

<style>
  .mpv-issue {
    position: absolute;
    bottom: calc(100% + 10px);
    left: 18px;
    z-index: 6;
    display: flex;
    align-items: flex-start;
    gap: 10px;
    max-width: min(620px, calc(100% - 36px));
    padding: 10px 14px;
    border: 1px solid color-mix(in srgb, var(--danger) 55%, var(--border));
    border-radius: var(--popover-radius);
    background: var(--dialog-bg);
    box-shadow: var(--popover-shadow);
    font-size: 13px;
  }

  .mpv-issue-icon {
    flex-shrink: 0;
    display: grid;
    place-items: center;
    color: var(--danger);
  }

  .text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-width: 0;
  }

  .mpv-issue-title {
    font-family: var(--meta-family);
    font-weight: 600;
    color: var(--danger);
  }

  .mpv-issue-hint {
    color: var(--text-muted);
    line-height: 1.4;
  }

  :global([data-theme="studio"]) .mpv-issue,
  :global([data-theme="atelier"]) .mpv-issue {
    border-radius: 0;
  }
</style>
