<script lang="ts">
  import { onMount } from "svelte";
  import { events } from "./lib/ipc";
  import { initPlayer, player } from "./lib/stores/player.svelte";
  import { initQueue } from "./lib/stores/queue.svelte";
  import { initToasts, noticeCrash, pushToast } from "./lib/stores/toasts.svelte";
  import { clearLikes, initLikes, loadLikes } from "./lib/stores/likes.svelte";
  import { initJam } from "./lib/stores/jam.svelte";
  import JamDialog from "./components/JamDialog.svelte";
  import { clearLater, initLater, loadLater } from "./lib/stores/later.svelte";
  import { clearFeed, initFeed } from "./lib/stores/feed.svelte";
  import { initUi, ui } from "./lib/stores/ui.svelte";
  import { initActions, runAction, setActionContext } from "./lib/actions";
  import { togglePalette } from "./lib/stores/palette.svelte";
  import CommandPalette from "./components/CommandPalette.svelte";
  import { initLanguage, t } from "./lib/i18n.svelte";
  import { openTrackInfo } from "./lib/stores/trackInfo.svelte";
  import { platform } from "./lib/platform.svelte";
  import { initWindow } from "./lib/stores/window.svelte";
  import { initBackButton } from "./lib/backButton";
  import { back, canGoBack, current, go, nav, openSettings } from "./lib/stores/nav.svelte";
  import { Shell } from "./app/shell.svelte";
  import { canClose, closeTop } from "./app/back";
  import { listenOutside, openShared } from "./app/outside";
  import { onContextmenu, onFocusChange, onKeydown, onPaste } from "./app/windowEvents";
  import Login from "./views/Login.svelte";
  import Home from "./views/Home.svelte";
  import Feed from "./views/Feed.svelte";
  import Search from "./views/Search.svelte";
  import Library from "./views/Library.svelte";
  import Stats from "./views/Stats.svelte";
  import Recap from "./views/Recap.svelte";
  import SmartList from "./views/SmartList.svelte";
  import BlindTest from "./views/BlindTest.svelte";
  import { initBlindTest } from "./lib/stores/blindTest.svelte";
  import Import from "./views/Import.svelte";
  import Folder from "./views/Folder.svelte";
  import Album from "./views/Album.svelte";
  import Artist from "./views/Artist.svelte";
  import Contributor from "./views/Contributor.svelte";
  import Playlist from "./views/Playlist.svelte";
  import Mix from "./views/Mix.svelte";
  import Queue from "./views/Queue.svelte";
  import NowPlaying from "./views/NowPlaying.svelte";
  import Drive from "./views/Drive.svelte";
  import Settings from "./views/Settings.svelte";
  import Backdrop from "./components/backdrops/Backdrop.svelte";
  import Sidebar from "./components/Sidebar.svelte";
  import PlayerBar from "./components/PlayerBar.svelte";
  import BottomNav from "./components/BottomNav.svelte";
  import MiniPlayer from "./components/MiniPlayer.svelte";
  import SelectionBar from "./components/SelectionBar.svelte";
  import { selection } from "./lib/stores/selection.svelte";
  import Toasts from "./components/Toasts.svelte";
  import DemoBanner from "./components/DemoBanner.svelte";
  import UpdatePrompt from "./components/UpdatePrompt.svelte";
  import ContextMenu from "./components/ContextMenu.svelte";
  import PlaylistPicker from "./components/PlaylistPicker.svelte";
  import FolderPicker from "./components/FolderPicker.svelte";
  import TrackInfo from "./components/TrackInfo.svelte";
  import Icon from "./components/Icon.svelte";
  import EggMark from "./components/EggMark.svelte";
  import EggLayer from "./components/EggLayer.svelte";
  import EndCard from "./components/EndCard.svelte";
  import { eggs, initEggs } from "./lib/eggs/eggs.svelte";
  import { romanVersion } from "./lib/eggs/registry";

  const shell = new Shell();
  let main = $state<HTMLElement>();

  // Action registry (#412): what it cannot do without `App` (panels, session).
  setActionContext({
    loggedIn: () => !!shell.session,
    openNowPlaying: () => (shell.nowPlayingOpen = true),
    closeNowPlaying: () => (shell.nowPlayingOpen = false),
    toggleQueue: () => (shell.queueOpen = !shell.queueOpen),
  });

  onMount(async () => {
    if (platform.mobile)
      initBackButton(
        () => canClose(shell),
        () => closeTop(shell),
      );
    void initActions();
    await Promise.all([
      initLanguage(),
      initUi(),
      initPlayer(),
      initQueue(),
      initToasts(),
      initLikes(),
      initLater(),
      initFeed(),
      initWindow(),
      initJam(),
      initBlindTest(),
    ]);
    initEggs();
    await shell.restoreSession();
    shell.ready = true;
    void noticeCrash();
    // Global "Like" shortcut (#413): run here, through the action registry.
    if (!platform.mobile) {
      void events.onGlobalShortcut((action) => {
        if (action === "like") runAction("play.like");
      });
    }
    void events.onSessionLost(() => {
      if (!shell.session) return;
      shell.session = null;
      clearLikes();
      clearLater();
      clearFeed();
      pushToast({ level: "error", message: t.views.login.sessionLost });
    });
    listenOutside(shell);
    if ((import.meta.env.DEV || __DEMO__) && !("__TAURI_INTERNALS__" in window)) {
      const view = (await import("./lib/mock")).initialView();
      if (view.route) go(view.route);
      shell.nowPlayingOpen = view.nowPlaying;
      shell.queueOpen = view.queue;
      ui.nowPlayingPanel =
        view.lyrics && view.queue ? "both" : view.lyrics ? "lyrics" : view.queue ? "queue" : null;
      if (view.trackInfo && player.state.track) openTrackInfo(player.state.track);
      shell.driveOpen = view.drive;
      if (view.palette) togglePalette();
    }
  });

  // Hearts: loaded at every sign-in (one request for the whole library).
  $effect(() => {
    if (shell.session) {
      loadLikes();
      loadLater();
    }
  });

  // Color of the current cover, used by the themes (`--album`).
  const albumColor = $derived(player.currentTrack?.album.color);
  $effect(() => {
    const color = albumColor;
    if (color) document.documentElement.style.setProperty("--album", color);
  });

  // Animated backgrounds (Acidwave) only move during playback.
  $effect(() => {
    document.documentElement.dataset.playing = String(player.playing);
  });

  // Every page change starts from the top.
  $effect(() => {
    void nav.stack.length;
    void current();
    main?.scrollTo({ top: 0 });
  });

  // Closed another way (Esc, shortcut, shared link): the mark must not outlive it.
  $effect(() => {
    if (!shell.nowPlayingOpen) shell.nowPlayingFromOutside = false;
  });

  const route = $derived(current());
</script>

<svelte:window
  onkeydown={(e) => onKeydown(shell, e)}
  onpaste={(e) => onPaste(shell, e)}
  oncontextmenu={onContextmenu}
  onfocusin={(e) => onFocusChange(shell, e)}
  onfocusout={(e) => onFocusChange(shell, e)}
/>

<div class="backdrop-layer"><Backdrop /></div>
<EggLayer />
<EndCard />

<div class="shell" class:mobile={platform.mobile}>
  <header>
    <!-- #399: searching "seaglass" sends a wave across the header once. -->
    {#if eggs.search === "seaglass"}<span class="egg-wave" aria-hidden="true">
        <span></span>
      </span>{/if}
    <div class="left">
      {#if shell.session || route.name === "settings"}
        <button
          class="back"
          onclick={back}
          disabled={!canGoBack()}
          aria-label={t.common.back}
          title={t.common.backHint}
        >
          <Icon name="previous" size={13} />
        </button>
      {/if}
      <!-- Name and version on the same baseline, centered together on the buttons (#183). -->
      <span class="wordmark">
        <span class="brand display">Seaglass<EggMark /></span>
        <!-- #399: on April 1st, the version is written in Roman numerals. -->
        <span class="version mono">
          v{eggs.date === "aprilFools" ? romanVersion(__APP_VERSION__) : __APP_VERSION__}
        </span>
        {#if eggs.date === "groundhog"}<span class="version mono egg-note">
            {t.eggs.groundhog}
          </span>{/if}
      </span>
    </div>
    {#if platform.mobile}
      <!-- Android: navigation at the bottom, settings at the top right (#143), sign-out in Settings › Application. -->
      {#if route.name !== "settings"}
        <div class="actions">
          {#if shell.session}
            <button
              class="icon-action"
              onclick={() => (shell.driveOpen = true)}
              aria-label={t.drive.open}
            >
              <Icon name="car" size={22} />
            </button>
          {/if}
          <button
            class="icon-action"
            onclick={() => openSettings()}
            aria-label={t.common.nav.settings}
          >
            <Icon name="settings" size={20} />
          </button>
        </div>
      {/if}
    {:else}
      <div class="actions">
        {#if shell.session}
          <button
            class="search-hint"
            onclick={() => (platform.mobile ? runAction("go.search") : togglePalette())}
          >
            {t.common.searchHint} <kbd>Ctrl K</kbd>
          </button>
        {/if}
        <button onclick={() => openSettings()} title={t.common.settingsHint}>
          {t.common.nav.settings}
        </button>
        <button onclick={() => openSettings("app")}>{t.common.about}</button>
      </div>
    {/if}
  </header>

  <div
    class="body"
    class:with-nav={shell.session && !platform.mobile}
    class:with-queue={shell.queueOpen && shell.session && !platform.mobile}
  >
    {#if shell.session && !platform.mobile}<div class="panel nav"><Sidebar /></div>{/if}
    <main class="panel" bind:this={main}>
      {#if !shell.ready}
        <!-- `.boot`: startup screen, which some themes decorate (Ship Computer, #339). -->
        <p class="muted boot">{t.common.loading}</p>
      {:else if route.name === "settings"}
        <Settings section={route.section} onLogout={shell.session ? shell.logout : undefined} />
      {:else if !shell.session}
        <Login
          restoreError={shell.restoreError}
          onLogin={(s) => {
            shell.signIn(s);
            if (platform.mobile) void openShared(shell);
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
      {:else if route.name === "stats"}
        <Stats />
      {:else if route.name === "recap"}
        <Recap />
      {:else if route.name === "smart"}
        <SmartList list={route.list} />
      {:else if route.name === "blindTest"}
        <BlindTest />
      {:else if route.name === "import"}
        {#key route.link}<Import link={route.link} />{/key}
      {:else if route.name === "folder"}
        <Folder id={route.id} />
      {:else if route.name === "album"}
        <Album id={route.id} track={route.track} />
      {:else if route.name === "artist"}
        <Artist id={route.id} />
      {:else if route.name === "contributor"}
        <Contributor id={route.id} />
      {:else if route.name === "playlist"}
        <Playlist uuid={route.uuid} />
      {:else if route.name === "mix"}
        <Mix source={{ mix: route.id }} />
      {:else if route.name === "radio"}
        <Mix source={{ seed: route.seed, id: route.id }} />
      {/if}
    </main>
    {#if shell.queueOpen && shell.session && !platform.mobile}
      <div class="panel queue"><Queue /></div>
    {/if}
  </div>

  {#if platform.mobile}
    {#if shell.session && !shell.typing}
      {#if selection.count > 0}
        <SelectionBar />
      {:else}
        <MiniPlayer onOpenNowPlaying={() => (shell.nowPlayingOpen = true)} />
      {/if}
      <div class="panel bottom"><BottomNav /></div>
    {/if}
  {:else}
    <div class="player">
      <PlayerBar
        queueOpen={shell.queueOpen}
        onToggleQueue={() => (shell.queueOpen = !shell.queueOpen)}
        onOpenNowPlaying={() => (shell.nowPlayingOpen = true)}
      />
    </div>
  {/if}
</div>

{#if shell.nowPlayingOpen}
  <NowPlaying onClose={shell.closeNowPlaying} />
{/if}
{#if shell.driveOpen && shell.session && platform.mobile}
  <Drive
    onClose={() => (shell.driveOpen = false)}
    onOpen={(target) => {
      shell.driveOpen = false;
      if (JSON.stringify(target) !== JSON.stringify(current())) go(target);
    }}
  />
{/if}
{#if shell.session && !platform.mobile && selection.count > 0 && !shell.nowPlayingOpen}
  <SelectionBar />
{/if}
<Toasts />
{#if __DEMO__}<DemoBanner />{/if}
<UpdatePrompt />
<ContextMenu />
<PlaylistPicker />
<FolderPicker />
<TrackInfo />
<JamDialog />
{#if shell.session && !platform.mobile}<CommandPalette />{/if}

<style>
  /* The theme's own background (empty for Studio and Atelier). */
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

  /* Android: the page goes under the system bars, the header and the navigation avoid them.
     Rows: header, page, mini player (missing without a track), navigation. */
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

  /* Android (#183): name centered in the bar, version below it. Centered on the header and
     not between its neighbors: Back and Settings are 36 px wide, and Settings is missing on
     its own page. */
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

  /* Page margins (24 px on desktop) brought down to a phone's width. */
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

  /* Round like the Back button, opposite it. */
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
    position: relative;
    font-size: 17px;
    color: var(--accent);
  }

  /* Easter eggs (#399): color of the name on some days, stronger than the themes'. */
  :global(:root[data-date-egg="halloween"]) .brand {
    color: #ff7a1a;
    -webkit-text-fill-color: #ff7a1a;
  }

  :global(:root[data-date-egg="jazz"]) .brand {
    color: #3f6fd8;
    -webkit-text-fill-color: #3f6fd8;
  }

  /* Searching "sorceleur": the name shakes briefly, like a medallion. */
  :global(:root[data-egg="witcher"]) .brand {
    animation: egg-shake 0.5s linear 2;
  }

  @keyframes egg-shake {
    0%,
    100% {
      translate: 0;
    }
    20%,
    60% {
      translate: -1.5px 0;
    }
    40%,
    80% {
      translate: 1.5px 0;
    }
  }

  /* December 21st: frost on the edges of the header. */
  :global(:root[data-date-egg="winter"]) header {
    box-shadow:
      inset 0 3px 6px -3px rgb(220 240 255 / 0.7),
      inset 0 -3px 6px -3px rgb(220 240 255 / 0.5);
  }

  header {
    position: relative;
  }

  .egg-wave {
    position: absolute;
    inset: 0;
    overflow: hidden;
    pointer-events: none;
    border-radius: inherit;
  }

  .egg-wave span {
    position: absolute;
    inset: 0 auto 0 0;
    width: 40%;
    background: linear-gradient(
      90deg,
      transparent,
      color-mix(in oklab, var(--accent) 22%, transparent) 45%,
      color-mix(in oklab, var(--accent) 35%, transparent) 50%,
      transparent
    );
    translate: -100% 0;
    animation: egg-wave 1.6s ease-in-out 1 forwards;
  }

  @keyframes egg-wave {
    to {
      translate: 260% 0;
    }
  }

  .shell.mobile .egg-note {
    display: none;
  }

  :global(:root[data-motion="reduced"]) .egg-wave {
    display: none;
  }

  :global(:root[data-motion="reduced"][data-egg="witcher"]) .brand {
    animation: none;
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

  /* Studio and Atelier: flat areas separated by rules. */
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
