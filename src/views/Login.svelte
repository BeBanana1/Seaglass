<script lang="ts">
  import { commands, type SessionInfo } from "../lib/ipc";

  let { onLogin }: { onLogin: (s: SessionInfo) => void } = $props();

  let pending = $state(false);
  let error = $state<string | null>(null);

  async function login() {
    pending = true;
    error = null;
    try {
      onLogin(await commands.login());
    } catch (e) {
      error = String(e);
    } finally {
      pending = false;
    }
  }
</script>

<section>
  <h1>Bienvenue</h1>
  <p>Connectez-vous avec votre compte de streaming pour commencer.</p>
  <button class="primary" onclick={login} disabled={pending}>
    {pending ? "Connexion en cours…" : "Se connecter"}
  </button>
  {#if error}
    <p class="error">{error}</p>
  {/if}
  <p class="disclaimer">Client non officiel, non affilié à TIDAL.</p>
</section>

<style>
  section {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 100%;
    text-align: center;
  }

  h1 {
    margin: 0;
    font-weight: 600;
  }

  p {
    margin: 0;
    color: var(--text-muted);
  }

  .primary {
    margin-top: 8px;
    padding: 10px 24px;
    border-radius: 999px;
    font-weight: 600;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .primary:hover:not(:disabled) {
    background: var(--accent-strong);
  }

  .primary:disabled {
    opacity: 0.6;
    cursor: default;
  }

  .error {
    max-width: 420px;
    color: var(--danger);
  }

  .disclaimer {
    margin-top: 24px;
    font-size: 12px;
  }
</style>
