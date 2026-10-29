<script lang="ts">
  let { onClose }: { onClose: () => void } = $props();
</script>

<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
<div class="backdrop" onclick={onClose}>
  <div class="dialog" role="dialog" tabindex="-1" aria-modal="true" aria-label="À propos" onclick={(e) => e.stopPropagation()}>
    <h2>Seaglass</h2>
    <p>Version {__APP_VERSION__}</p>
    <p>Licence GPL-3.0.</p>
    <p><strong>Client non officiel, non affilié à TIDAL.</strong></p>
    <p>Streaming uniquement : aucun flux n'est téléchargé ni enregistré sur disque.</p>
    <button onclick={onClose}>Fermer</button>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    z-index: 60;
    display: grid;
    place-items: center;
    background: rgb(0 0 0 / 0.5);
  }

  .dialog {
    width: min(400px, 90vw);
    padding: 24px;
    border: 1px solid var(--border);
    border-radius: calc(var(--panel-radius) + 2px);
    color: var(--text);
    background: var(--dialog-bg);
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.4);
  }

  h2 {
    margin: 0 0 8px;
  }

  p {
    margin: 8px 0;
    color: var(--text-muted);
  }

  strong {
    color: var(--text);
  }

  button {
    margin-top: 12px;
    padding: 6px 16px;
    border: 1px solid var(--border);
    border-radius: 999px;
  }

  button:hover {
    background: var(--bg-hover);
  }
</style>
