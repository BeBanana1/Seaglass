<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type SessionInfo } from "./lib/ipc";
  import { initPlayer, player, stepVolume } from "./lib/stores/player.svelte";
  import { initQueue } from "./lib/stores/queue.svelte";
  import { initToasts, pushToast } from "./lib/stores/toasts.svelte";
  import { initUi, ui } from "./lib/stores/ui.svelte";
  import { back, canGoBack, current, go, nav, openSettings, replace } from "./lib/stores/nav.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Feed from "./views/Feed.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import Album from "./views/Album.svelte";
  import Artist from "./views/Artist.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Queue from "./views/Queue.svelte";
  import NowPlaying from "./views/NowPlaying.svelte";
  import Settings from "./views/Settings.svelte";
  import Backdrop from "./components/backdrops/Backdrop.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdatePrompt from "./components/UpdatePrompt.svelte";
  import Icon from "./components/Icon.svelte";

  let session = $state<SessionInfo | null>(null);
  let ready = $state(false);
  let queueOpen = $state(false);
  let nowPlayingOpen = $state(false);
  let main = $state<HTMLElement>();

  onMount(async () => {
    await Promise.all([initUi(), initPlayer(), initQueue(), initToasts()]);
    try {
      session = await commands.authStatus();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    ready = true;
    if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
      const view = (await import("./lib/mock")).initialView();
      if (view.route) go(view.route);
      nowPlayingOpen = view.nowPlaying;
      queueOpen = view.queue;
      ui.nowPlayingQueue = view.queue;
    }
  });

  // Couleur de la pochette en cours, reprise par les thèmes (`--album`).
  $effect(() => {
    const color = player.state.track?.album.color;
    if (color) document.documentElement.style.setProperty("--album", color);
  });

  // Les fonds animés (Acidwave) ne bougent que pendant la lecture.
  $effect(() => {
    const s = player.state;
    document.documentElement.dataset.playing = String(!!s.track && !s.paused && !s.idle);
  });

  // Chaque changement de page repart du haut.
  $effect(() => {
    void nav.stack.length;
    void current();
    main?.scrollTo({ top: 0 });
  });

  function onKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    const typing = target.closest("input, textarea, [contenteditable]") !== null;
    if (e.key === "Escape") {
      if (nowPlayingOpen) nowPlayingOpen = false;
      return;
    }
    // Les réglages restent accessibles déconnecté (thème, cache).
    if (e.ctrlKey && e.key === ",") {
      e.preventDefault();
      nowPlayingOpen = false;
      openSettings();
      return;
    }
    if (!session) return;
    if ((e.ctrlKey && e.key.toLowerCase() === "k") || (e.key === "/" && !typing)) {
      e.preventDefault();
      nowPlayingOpen = false;
      if (current().name !== "search") replace({ name: "search" });
      requestAnimationFrame(() =>
        document.querySelector<HTMLInputElement>('input[type="search"]')?.focus(),
      );
    } else if (e.key === " " && !typing && !target.closest("button")) {
      e.preventDefault();
      commands.togglePause();
    } else if (e.altKey && e.key === "ArrowLeft") {
      back();
    } else if (e.ctrlKey && (e.key === "ArrowUp" || e.key === "ArrowDown") && !typing) {
      e.preventDefault();
      stepVolume(e.key === "ArrowUp" ? 1 : -1);
    }
  }

  async function logout() {
    await commands.logout();
    session = null;
  }

  const route = $derived(current());
</script>

<svelte:window onkeydown={onKeydown} />

<div class="backdrop-layer"><Backdrop /></div>

<div class="shell">
  <header>
    <div class="left">
      {#if session || route.name === "settings"}
        <button class="back" onclick={back} disabled={!canGoBack()} aria-label="Retour" title="Retour (Alt+←)">
          <Icon name="previous" size={13} />
        </button>
      {/if}
      <span class="brand display">Seaglass</span>
      <span class="version mono">v{__APP_VERSION__}</span>
    </div>
    <div class="actions">
      {#if session}
        <button class="search-hint" onclick={() => replace({ name: "search" })}>
          Rechercher <kbd>Ctrl K</kbd>
        </button>
      {/if}
      <button onclick={() => openSettings()} title="Réglages (Ctrl+,)">Réglages</button>
      <button onclick={() => openSettings("app")}>À propos</button>
      {#if session}
        <button onclick={logout}>Se déconnecter</button>
      {/if}
    </div>
  </header>

  <div class="body" class:with-nav={session} class:with-queue={queueOpen && session}>
    {#if session}<div class="panel nav"><Sidebar /></div>{/if}
    <main class="panel" bind:this={main}>
      {#if !ready}
        <p class="muted">Chargement…</p>
      {:else if route.name === "settings"}
        <Settings section={route.section} />
      {:else if !session}
        <Login onLogin={(s) => (session = s)} />
      {:else if route.name === "home"}
        <Home />
      {:else if route.name === "feed"}
        <Feed />
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
      <div class="panel queue"><Queue /></div>
    {/if}
  </div>

  <div class="player">
    <PlayerBar
      {queueOpen}
      onToggleQueue={() => (queueOpen = !queueOpen)}
      onOpenNowPlaying={() => (nowPlayingOpen = true)}
    />
  </div>
</div>

{#if nowPlayingOpen}
  <NowPlaying onClose={() => (nowPlayingOpen = false)} />
{/if}
<Toasts />
<UpdatePrompt />

<style>
  /* Fond propre au thème (vide pour Studio et Atelier). */
  .backdrop-layer {
    position: fixed;
    inset: 0;
    z-index: -1;
  }

  .shell {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) var(--player-height);
    gap: var(--shell-gap);
    height: 100%;
    padding: var(--shell-gap);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px;
  }

  .left {
    display: flex;
    align-items: baseline;
    gap: 10px;
  }

  .back {
    align-self: center;
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
    opacity: 0.35;
    cursor: default;
  }

  .brand {
    font-size: 17px;
    color: var(--accent);
  }

  .version {
    font-size: 11px;
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 18px;
    color: var(--text-muted);
  }

  .actions button:hover {
    color: var(--text);
  }

  .search-hint {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 5px 8px 5px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg-raised);
  }

  kbd {
    padding: 1px 6px;
    border-radius: 4px;
    font-family: var(--font-mono);
    font-size: 10px;
    background: var(--bg-hover);
  }

  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    gap: var(--shell-gap);
    min-height: 0;
  }

  .body.with-nav {
    grid-template-columns: 210px minmax(0, 1fr);
  }

  .body.with-nav.with-queue {
    grid-template-columns: 210px minmax(0, 1fr) 340px;
  }

  main {
    overflow: auto;
    min-height: 0;
  }

  .nav,
  .queue {
    min-height: 0;
    overflow: hidden;
  }

  .player {
    min-height: 0;
  }

  .player :global(footer) {
    height: 100%;
  }

  .muted {
    padding: 20px;
    color: var(--text-muted);
  }

  /* Studio et Atelier : zones à plat séparées par des filets. */
  :global([data-theme="studio"]) header,
  :global([data-theme="atelier"]) header {
    padding: 10px 18px;
    border-bottom: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .nav,
  :global([data-theme="atelier"]) .nav {
    border-right: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .queue,
  :global([data-theme="atelier"]) .queue {
    border-left: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .brand {
    font-family: var(--font-mono);
    font-size: 13px;
    letter-spacing: 0.2em;
    text-transform: uppercase;
  }

  :global([data-theme="studio"]) .back,
  :global([data-theme="studio"]) .search-hint,
  :global([data-theme="studio"]) kbd {
    border-radius: 0;
  }

  :global([data-theme="atelier"]) .brand {
    font-size: 22px;
    font-style: italic;
    color: var(--text);
  }

  :global([data-theme="atelier"]) .search-hint {
    background: transparent;
  }
</style>
