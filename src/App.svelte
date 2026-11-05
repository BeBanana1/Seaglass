<script lang="ts">
  import { onMount } from "svelte";
  import { commands, events, type SessionInfo } from "./lib/ipc";
  import { initPlayer, player, stepVolume } from "./lib/stores/player.svelte";
  import { initQueue } from "./lib/stores/queue.svelte";
  import { initToasts, pushToast } from "./lib/stores/toasts.svelte";
  import { clearLikes, initLikes, loadLikes } from "./lib/stores/likes.svelte";
  import { initUi, ui } from "./lib/stores/ui.svelte";
  import { initLanguage, t } from "./lib/i18n.svelte";
  import { closeTrackInfo, openTrackInfo, trackInfo } from "./lib/stores/trackInfo.svelte";
  import { closeMenu, menu } from "./lib/stores/menu.svelte";
  import { closePlaylistPicker, picker } from "./lib/stores/playlistPicker.svelte";
  import { platform } from "./lib/platform.svelte";
  import { appWindow, initWindow, setFullscreen, toggleFullscreen } from "./lib/stores/window.svelte";
  import { initBackButton, syncBackButton } from "./lib/backButton";
  import { exitListSearch, listSearch } from "./lib/stores/listSearch.svelte";
  import { openSharedLink, openTidalLink } from "./lib/links";
  import { back, canGoBack, current, go, nav, openSettings, replace } from "./lib/stores/nav.svelte";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Feed from "./views/Feed.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import Album from "./views/Album.svelte";
  import Artist from "./views/Artist.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Mix from "./views/Mix.svelte";
  import Queue from "./views/Queue.svelte";
  import NowPlaying from "./views/NowPlaying.svelte";
  import Settings from "./views/Settings.svelte";
  import Backdrop from "./components/backdrops/Backdrop.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import BottomNav from "./components/BottomNav.svelte";
  import MiniPlayer from "./components/MiniPlayer.svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdatePrompt from "./components/UpdatePrompt.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import PlaylistPicker from "./components/PlaylistPicker.svelte";
  import TrackInfo from "./components/TrackInfo.svelte";
  import Icon from "./components/Icon.svelte";

  let session = $state<SessionInfo | null>(null);
  let ready = $state(false);
  let queueOpen = $state(false);
  let nowPlayingOpen = $state(false);
  let main = $state<HTMLElement>();
  // Android : un champ de texte a le focus, donc le clavier est ouvert (#151).
  let typing = $state(false);

  onMount(async () => {
    if (platform.mobile) initBackButton(canClose, closeTop);
    await Promise.all([initLanguage(), initUi(), initPlayer(), initQueue(), initToasts(), initLikes(), initWindow()]);
    try {
      session = await commands.authStatus();
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    ready = true;
    if (platform.mobile) {
      void events.onOpenNowPlaying(openFromNotification);
      void openFromNotification();
      void events.onSharedLink(openShared);
      void openShared();
    }
    if (import.meta.env.DEV && !("__TAURI_INTERNALS__" in window)) {
      const view = (await import("./lib/mock")).initialView();
      if (view.route) go(view.route);
      nowPlayingOpen = view.nowPlaying;
      queueOpen = view.queue;
      ui.nowPlayingPanel =
        view.lyrics && view.queue ? "both" : view.lyrics ? "lyrics" : view.queue ? "queue" : null;
      if (view.trackInfo && player.state.track) openTrackInfo(player.state.track);
    }
  });

  // Cœurs : chargés à chaque connexion (une requête pour toute la bibliothèque).
  $effect(() => {
    if (session) loadLikes();
  });

  // Couleur de la pochette en cours, reprise par les thèmes (`--album`).
  const albumColor = $derived(player.currentTrack?.album.color);
  $effect(() => {
    const color = albumColor;
    if (color) document.documentElement.style.setProperty("--album", color);
  });

  // Les fonds animés (Acidwave) ne bougent que pendant la lecture.
  $effect(() => {
    document.documentElement.dataset.playing = String(player.playing);
  });

  // Chaque changement de page repart du haut.
  $effect(() => {
    void nav.stack.length;
    void current();
    main?.scrollTo({ top: 0 });
  });

  // Retour d'Android : ferme ce qui est au-dessus (menu, dialogue, « En lecture »), quitte
  // la recherche dans une liste, puis revient à la page précédente.
  function canClose(): boolean {
    return (
      !!menu.open || !!picker.request || !!trackInfo.track || nowPlayingOpen || listSearch.active || canGoBack()
    );
  }

  function closeTop() {
    if (menu.open) closeMenu();
    else if (picker.request) closePlaylistPicker();
    else if (trackInfo.track) closeTrackInfo();
    // Paroles ou file en plein écran (#173) : Retour revient d'abord à « En lecture ».
    else if (nowPlayingOpen && ui.nowPlayingPanel) ui.nowPlayingPanel = null;
    else if (nowPlayingOpen) nowPlayingOpen = false;
    else if (listSearch.active) exitListSearch();
    else back();
  }

  // Android : mini-lecteur et navigation masqués pendant la saisie, comme dans les apps
  // natives ; ils prendraient sinon la place des résultats au-dessus du clavier (#151).
  const NOT_TEXT = new Set(["button", "checkbox", "color", "file", "radio", "range", "reset", "submit"]);

  // Appui sur la notification d'Android (#163) : demande gardée par Rust, prise une seule
  // fois (aussi au démarrage, si l'app a été rouverte par la notification).
  async function openFromNotification() {
    if (!(await commands.takeNowPlayingRequest().catch(() => false))) return;
    if (session && player.state.track) nowPlayingOpen = true;
  }

  // Lien partagé depuis une autre app (#203) : seulement connecté, sinon il attend.
  async function openShared() {
    if (!session) return;
    nowPlayingOpen = false;
    await openSharedLink();
  }

  function isTextField(el: EventTarget | null): boolean {
    if (el instanceof HTMLTextAreaElement) return true;
    return el instanceof HTMLInputElement && !NOT_TEXT.has(el.type);
  }

  function onFocusChange(e: FocusEvent) {
    if (!platform.mobile) return;
    typing = isTextField(e.type === "focusin" ? e.target : e.relatedTarget);
  }

  $effect(() => {
    if (!platform.mobile) return;
    void canClose();
    syncBackButton();
  });

  function onKeydown(e: KeyboardEvent) {
    const target = e.target as HTMLElement;
    const typing = target.closest("input, textarea, [contenteditable]") !== null;
    // Plein écran de la fenêtre (#131), indépendant de « En lecture » ; Échap en sort
    // d'abord, comme une vidéo, puis ferme la vue au second appui.
    if (e.key === "F11" && !platform.mobile) {
      e.preventDefault();
      toggleFullscreen();
      return;
    }
    if (e.key === "Escape") {
      if (appWindow.fullscreen) setFullscreen(false);
      else if (nowPlayingOpen) nowPlayingOpen = false;
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

  // Pas de menu WebView2 (Précédent, Actualiser, Imprimer…) hors des champs de texte et des
  // sélections, où il sert à copier-coller. Les menus de l'app appellent `preventDefault` avant.
  function onContextmenu(e: MouseEvent) {
    if (e.defaultPrevented) return;
    const target = e.target as HTMLElement;
    const editable = target.closest("input, textarea, [contenteditable]") !== null;
    const selected = (window.getSelection()?.toString() ?? "") !== "";
    if (!editable && !selected) e.preventDefault();
  }

  async function logout() {
    await commands.logout();
    session = null;
    clearLikes();
  }

  const route = $derived(current());

  // Lien TIDAL collé hors d'un champ de texte (#203) : ouvre sa page.
  function onPaste(e: ClipboardEvent) {
    if (!session) return;
    const target = e.target as HTMLElement | null;
    if (target?.closest?.("input, textarea, [contenteditable]")) return;
    const text = e.clipboardData?.getData("text/plain");
    if (text) void openTidalLink(text);
  }
</script>

<svelte:window
  onkeydown={onKeydown}
  onpaste={onPaste}
  oncontextmenu={onContextmenu}
  onfocusin={onFocusChange}
  onfocusout={onFocusChange}
/>

<div class="backdrop-layer"><Backdrop /></div>

<div class="shell" class:mobile={platform.mobile}>
  <header>
    <div class="left">
      {#if session || route.name === "settings"}
        <button class="back" onclick={back} disabled={!canGoBack()} aria-label={t.common.back} title={t.common.backHint}>
          <Icon name="previous" size={13} />
        </button>
      {/if}
      <!-- Nom et version sur une même ligne de base, centrés ensemble sur les boutons (#183). -->
      <span class="wordmark">
        <span class="brand display">Seaglass</span>
        <span class="version mono">v{__APP_VERSION__}</span>
      </span>
    </div>
    {#if platform.mobile}
      <!-- Android : navigation en bas, réglages en haut à droite (#143), déconnexion dans Réglages › Application. -->
      {#if route.name !== "settings"}
        <div class="actions">
          <button class="icon-action" onclick={() => openSettings()} aria-label={t.common.nav.settings}>
            <Icon name="settings" size={20} />
          </button>
        </div>
      {/if}
    {:else}
    <div class="actions">
      {#if session}
        <button class="search-hint" onclick={() => replace({ name: "search" })}>
          {t.common.searchHint} <kbd>Ctrl K</kbd>
        </button>
      {/if}
      <button onclick={() => openSettings()} title={t.common.settingsHint}>{t.common.nav.settings}</button>
      <button onclick={() => openSettings("app")}>{t.common.about}</button>
    </div>
    {/if}
  </header>

  <div class="body" class:with-nav={session && !platform.mobile} class:with-queue={queueOpen && session && !platform.mobile}>
    {#if session && !platform.mobile}<div class="panel nav"><Sidebar /></div>{/if}
    <main class="panel" bind:this={main}>
      {#if !ready}
        <p class="muted">{t.common.loading}</p>
      {:else if route.name === "settings"}
        <Settings section={route.section} onLogout={session ? logout : undefined} />
      {:else if !session}
        <Login
          onLogin={(s) => {
            session = s;
            if (platform.mobile) void openShared();
          }}
        />
      {:else if route.name === "home"}
        <Home />
      {:else if route.name === "feed"}
        <Feed />
      {:else if route.name === "search"}
        <Search />
      {:else if route.name === "library"}
        <Library tab={route.tab} />
      {:else if route.name === "album"}
        <Album id={route.id} track={route.track} />
      {:else if route.name === "artist"}
        <Artist id={route.id} />
      {:else if route.name === "playlist"}
        <Playlist uuid={route.uuid} />
      {:else if route.name === "mix"}
        <Mix source={{ mix: route.id }} />
      {:else if route.name === "radio"}
        <Mix source={{ seed: route.seed, id: route.id }} />
      {/if}
    </main>
    {#if queueOpen && session && !platform.mobile}
      <div class="panel queue"><Queue /></div>
    {/if}
  </div>

  {#if platform.mobile}
    {#if session && !typing}
      <MiniPlayer onOpenNowPlaying={() => (nowPlayingOpen = true)} />
      <div class="panel bottom"><BottomNav /></div>
    {/if}
  {:else}
    <div class="player">
      <PlayerBar
        {queueOpen}
        onToggleQueue={() => (queueOpen = !queueOpen)}
        onOpenNowPlaying={() => (nowPlayingOpen = true)}
      />
    </div>
  {/if}
</div>

{#if nowPlayingOpen}
  <NowPlaying onClose={() => (nowPlayingOpen = false)} />
{/if}
<Toasts />
<UpdatePrompt />
<ContextMenu />
<PlaylistPicker />
<TrackInfo />

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

  /* Android : la page passe sous les barres système, l'en-tête et la navigation les évitent.
     Lignes : en-tête, page, mini-lecteur (absent sans titre), navigation. */
  .shell.mobile {
    --mobile-gap: min(var(--shell-gap), 6px);
    display: flex;
    flex-direction: column;
    gap: var(--mobile-gap);
    padding: calc(var(--safe-top) + var(--mobile-gap)) var(--mobile-gap) 0;
  }

  .shell.mobile .body {
    flex: 1;
  }

  .shell.mobile header {
    position: relative;
    min-height: 48px;
    padding: 0 8px;
  }

  /* Android (#183) : nom centré dans la barre, version dessous. Centré sur l'en-tête et non
     entre ses voisins : Retour et Réglages font 36 px, et Réglages manque sur sa propre page. */
  .shell.mobile .wordmark {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    flex-direction: column;
    align-items: center;
    gap: 0;
    line-height: 1.15;
    pointer-events: none;
  }

  .shell.mobile .version {
    font-size: 10px;
  }

  .shell.mobile .back {
    width: 36px;
    height: 36px;
  }

  /* Marges des pages (24 px sur le bureau) ramenées à une largeur de téléphone. */
  .shell.mobile main > :global(*) {
    padding: 16px 12px;
  }

  .shell.mobile .bottom {
    margin: 0 calc(-1 * var(--mobile-gap));
    border-right: none;
    border-bottom: none;
    border-left: none;
    border-radius: var(--panel-radius) var(--panel-radius) 0 0;
  }

  /* Rond comme le bouton Retour, en face de lui. */
  .icon-action {
    display: grid;
    place-items: center;
    width: 36px;
    height: 36px;
    border-radius: 50%;
    color: var(--text-muted);
    background: var(--bg-raised);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 4px 8px;
  }

  .left {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .wordmark {
    display: flex;
    align-items: baseline;
    gap: 10px;
    white-space: nowrap;
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

  .actions button {
    clip-path: var(--control-clip);
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
