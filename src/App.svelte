<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type SessionInfo } from "./lib/ipc";
  import { initPlayer } from "./lib/stores/player.svelte";
  import { initToasts, pushToast } from "./lib/stores/toasts.svelte";
  import Login from "./views/Login.svelte";
  import Search from "./views/Search.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import About from "./components/About.svelte";

  let session = $state<SessionInfo | null>(null);
  let ready = $state(false);
  let aboutOpen = $state(false);

  onMount(async () => {
    await Promise.all([initPlayer(), initToasts()]);
    try {
      session = await commands.authStatus();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    ready = true;
  });

  async function logout() {
    await commands.logout();
    session = null;
  }
</script>

<div class="shell">
  <header>
    <span class="brand">Seaglass</span>
    <div class="actions">
      <button onclick={() => (aboutOpen = true)}>À propos</button>
      {#if session}
        <button onclick={logout}>Se déconnecter</button>
      {/if}
    </div>
  </header>

  <main>
    {#if !ready}
      <p class="muted">Chargement…</p>
    {:else if session}
      <Search />
    {:else}
      <Login onLogin={(s) => (session = s)} />
    {/if}
  </main>

  <PlayerBar />
</div>

<Toasts />
{#if aboutOpen}
  <About onClose={() => (aboutOpen = false)} />
{/if}

<style>
  .shell {
    display: grid;
    grid-template-rows: auto 1fr var(--player-height);
    height: 100%;
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 12px 20px;
    border-bottom: 1px solid var(--border);
  }

  .brand {
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--accent);
  }

  .actions {
    display: flex;
    gap: 16px;
    color: var(--text-muted);
  }

  .actions button:hover {
    color: var(--text);
  }

  main {
    overflow: auto;
    min-height: 0;
  }

  .muted {
    padding: 20px;
    color: var(--text-muted);
  }
</style>
