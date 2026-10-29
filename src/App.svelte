<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type SessionInfo } from "./lib/ipc";
  import { initPlayer } from "./lib/stores/player.svelte";
  import { initQueue } from "./lib/stores/queue.svelte";
  import { initToasts, pushToast } from "./lib/stores/toasts.svelte";
  import { back, canGoBack, current, nav } from "./lib/stores/nav.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import Album from "./views/Album.svelte";
  import Artist from "./views/Artist.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Queue from "./views/Queue.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import About from "./components/About.svelte";
  import Icon from "./components/Icon.svelte";

  let session = $state<SessionInfo | null>(null);
  let ready = $state(false);
  let aboutOpen = $state(false);
  let queueOpen = $state(false);
  let main = $state<HTMLElement>();

  onMount(async () => {
    await Promise.all([initPlayer(), initQueue(), initToasts()]);
    try {
      session = await commands.authStatus();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    ready = true;
  });

  // Chaque changement de page repart du haut.
  $effect(() => {
    void nav.stack.length;
    void current();
    main?.scrollTo({ top: 0 });
  });

  async function logout() {
    await commands.logout();
    session = null;
  }

  const route = $derived(current());
</script>

<div class="shell">
  <header>
    <div class="left">
      {#if session}
        <button class="back" onclick={back} disabled={!canGoBack()} aria-label="Retour">
          <Icon name="previous" size={14} />
        </button>
      {/if}
      <span class="brand">Seaglass <span class="version">v{__APP_VERSION__}</span></span>
    </div>
    <div class="actions">
      <button onclick={() => (aboutOpen = true)}>À propos</button>
      {#if session}
        <button onclick={logout}>Se déconnecter</button>
      {/if}
    </div>
  </header>

  <div class="body" class:with-nav={session} class:with-queue={queueOpen && session}>
    {#if session}<Sidebar />{/if}
    <main bind:this={main}>
      {#if !ready}
        <p class="muted">Chargement…</p>
      {:else if !session}
        <Login onLogin={(s) => (session = s)} />
      {:else if route.name === "home"}
        <Home />
      {:else if route.name === "search"}
        <Search />
      {:else if route.name === "library"}
        <Library tab={route.tab} />
      {:else if route.name === "album"}
        <Album id={route.id} />
      {:else if route.name === "artist"}
        <Artist id={route.id} />
      {:else if route.name === "playlist"}
        <Playlist uuid={route.uuid} />
      {/if}
    </main>
    {#if queueOpen && session}
      <Queue />
    {/if}
  </div>

  <PlayerBar {queueOpen} onToggleQueue={() => (queueOpen = !queueOpen)} />
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
    padding: 10px 20px;
    border-bottom: 1px solid var(--border);
  }

  .left {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .back {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  .back:hover:not(:disabled) {
    color: var(--text);
  }

  .back:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .brand {
    font-weight: 600;
    letter-spacing: 0.02em;
    color: var(--accent);
  }

  .version {
    margin-left: 6px;
    font-size: 11px;
    font-weight: 400;
    letter-spacing: 0;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    gap: 16px;
    color: var(--text-muted);
  }

  .actions button:hover {
    color: var(--text);
  }

  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    min-height: 0;
  }

  .body.with-nav {
    grid-template-columns: 200px minmax(0, 1fr);
  }

  .body.with-nav.with-queue {
    grid-template-columns: 200px minmax(0, 1fr) 340px;
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
