<script lang="ts">
  // Immersive "Now playing" view: large cover, signal path, up next,
  // and a collapsible side panel: queue (#20) or lyrics (#5).
  import { untrack } from "svelte";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { commands, type Track } from "../lib/ipc";
  import { ui } from "../lib/stores/ui.svelte";
  import { coverLarge, coverSmall, formatStream, fullTitle, signalVerdict } from "../lib/format";
  import { copyCard } from "../lib/shareCard";
  import { go, nav } from "../lib/stores/nav.svelte";
  import { openPlaylistPicker } from "../lib/stores/playlistPicker.svelte";
  import { openTrackInfo } from "../lib/stores/trackInfo.svelte";
  import { jam, jamAvailable, openJamDialog } from "../lib/stores/jam.svelte";
  import {
    handoff,
    loadLinkedComputer,
    pullHere,
    pushToComputer,
  } from "../lib/stores/handoff.svelte";
  import { playedFrom } from "../lib/playSource";
  import Backdrop from "../components/backdrops/Backdrop.svelte";
  import Icon from "../components/Icon.svelte";
  import LikeButton from "../components/LikeButton.svelte";
  import Lyrics from "../components/Lyrics.svelte";
  import Progress from "../components/Progress.svelte";
  import QueueList from "../components/QueueList.svelte";
  import SignalPath from "../components/SignalPath.svelte";
  import SleepTimer from "../components/SleepTimer.svelte";
  import Transport from "../components/Transport.svelte";
  import Volume from "../components/Volume.svelte";
  import TrackRow from "../components/TrackRow.svelte";
  import { platform } from "../lib/platform.svelte";
  import { appWindow, toggleFullscreen } from "../lib/stores/window.svelte";
  import { t } from "../lib/i18n.svelte";
  import { closeMenu, menu, openMenu } from "../lib/stores/menu.svelte";
  import { trackMenu } from "../lib/trackMenu";

  let { onClose }: { onClose: () => void } = $props();

  const current = $derived(player.currentTrack);
  const stream = $derived(player.state.stream);
  /** Other hi-res edition actually played (#111). */
  const edition = $derived(stream?.hiresEdition ?? null);
  const upNext = $derived([...queue.view.manual, ...queue.view.upcoming].slice(0, 5));

  /** Plays track `i` of `upNext` (#167): manual queue first, then the rest of the context. */
  async function playUpNext(i: number, track: Track) {
    const manual = queue.view.manual.length;
    try {
      if (i < manual) await commands.playFromQueue("manual", i, track.id);
      else await commands.playFromQueue("upcoming", i - manual, track.id);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
  const panel = $derived(ui.nowPlayingPanel);
  const from = $derived(playedFrom(queue.view.source));

  // Wide window (#119): lyrics and queue side by side, independent buttons. Elsewhere, one
  // panel at a time; "both" (window made narrower) keeps the lyrics, with the "Up next" strip.
  const WIDE = "(min-width: 1400px)";
  let wide = $state(!platform.mobile && matchMedia(WIDE).matches);
  $effect(() => {
    if (platform.mobile) return;
    const query = matchMedia(WIDE);
    const onChange = () => (wide = query.matches);
    query.addEventListener("change", onChange);
    return () => query.removeEventListener("change", onChange);
  });

  const lyricsOpen = $derived(panel === "lyrics" || panel === "both");
  const queueOpen = $derived(panel === "queue" || (panel === "both" && wide));
  const bothOpen = $derived(lyricsOpen && queueOpen);
  const anyOpen = $derived(lyricsOpen || queueOpen);
  /** Android (#173): an open panel replaces the whole view instead of being added below. */
  const sheet = $derived(platform.mobile && anyOpen);
  /** #399, #11: widescreen bars for a hand-picked soundtrack. */
  const scope = $derived(player.soundtrack && ui.settings.easterEggs);

  let moreButton = $state<HTMLButtonElement | null>(null);

  // #414: handoff with the linked computer (Android).
  let handoffButton = $state<HTMLButtonElement>();
  $effect(() => {
    if (platform.mobile) untrack(loadLinkedComputer);
  });

  function toggleHandoff() {
    if (!handoffButton) return;
    if (menu.open?.from === handoffButton) return closeMenu();
    const rect = handoffButton.getBoundingClientRect();
    openMenu({
      x: rect.left,
      y: rect.bottom + 4,
      entries: [
        { label: t.handoff.pull, icon: "devices", action: pullHere },
        { label: t.handoff.push, icon: "devices", action: pushToComputer },
      ],
      from: handoffButton,
    });
  }

  /** "Now playing" card as a PNG in the clipboard (#518), with the format actually played. */
  async function shareImage(track: Track) {
    const s = player.state;
    const verdict = signalVerdict(
      s.stream,
      s.deviceFormat ?? s.output,
      s.volume,
      s.normalization,
      s.exclusive,
      s.equalizer,
    );
    try {
      await copyCard(track, s.stream, verdict);
      pushToast({ level: "info", message: t.menu.imageCopied });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }

  function toggleMore(track: Track) {
    if (!moreButton) return;
    if (menu.open?.from === moreButton) return closeMenu();
    const rect = moreButton.getBoundingClientRect();
    const entries = trackMenu(track, {
      queueable: false,
      // The blocked track (or the blocked artist's) is the one playing: skip it.
      onBlocked: () =>
        commands.next().catch((e) => pushToast({ level: "error", message: String(e) })),
      // Desktop only for now: Android has no outgoing share sheet yet (#518).
      onShareImage: platform.mobile ? undefined : () => shareImage(track),
    });
    openMenu({ x: rect.left, y: rect.bottom + 4, entries, from: moreButton });
  }

  function toggle(p: "queue" | "lyrics") {
    let lyrics = lyricsOpen;
    let queueShown = queueOpen;
    if (p === "lyrics") lyrics = !lyrics;
    else queueShown = !queueShown;
    // Narrow window: opening a panel closes the other one.
    if (!wide && lyrics && queueShown) {
      if (p === "lyrics") queueShown = false;
      else lyrics = false;
    }
    ui.nowPlayingPanel =
      lyrics && queueShown ? "both" : lyrics ? "lyrics" : queueShown ? "queue" : null;
  }

  // Any navigation (artist or album link, here or in the queue) closes the view,
  // otherwise the opened page would stay hidden behind it.
  const stackAtOpen = untrack(() => nav.stack);

  // Android (#173): reopening "Now playing" (mini player, notification) shows the cover and
  // the controls, not the last fullscreen panel.
  $effect(() => () => {
    if (platform.mobile) ui.nowPlayingPanel = null;
  });
  $effect(() => {
    if (nav.stack !== stackAtOpen) onClose();
  });
</script>

<div
  class="now-playing"
  class:lyrics-open={lyricsOpen && !queueOpen}
  class:both-open={bothOpen}
  class:scope
  role="dialog"
  aria-modal="true"
  aria-label={t.views.nowPlaying.title}
>
  <Backdrop context="nowplaying" />
  <!-- #399, #11: hand-picked soundtrack (backend list), the view goes widescreen. The bars stay
       mounted so they can slide out as well as in. -->
  {#if ui.settings.easterEggs}<div class="egg-scope" aria-hidden="true"></div>{/if}
  <div class="top-actions" class:with-panel={anyOpen}>
    <button
      class="lyrics-toggle"
      class:on={lyricsOpen}
      onclick={() => toggle("lyrics")}
      aria-pressed={lyricsOpen}
      title={t.views.nowPlaying.lyrics}
    >
      <Icon name="lyrics" size={20} />
    </button>
    <button
      class="queue-toggle"
      class:on={queueOpen}
      onclick={() => toggle("queue")}
      aria-pressed={queueOpen}
      title={t.player.queue}
    >
      <Icon name="queue" size={20} />
    </button>
    {#if !platform.mobile}
      <button
        class="fullscreen-toggle"
        onclick={toggleFullscreen}
        aria-pressed={appWindow.fullscreen}
        title={appWindow.fullscreen
          ? t.views.nowPlaying.exitFullscreen
          : t.views.nowPlaying.fullscreen}
        aria-label={appWindow.fullscreen
          ? t.views.nowPlaying.exitFullscreen
          : t.views.nowPlaying.fullscreen}
      >
        <Icon name={appWindow.fullscreen ? "fullscreenExit" : "fullscreen"} size={20} />
      </button>
    {/if}
    <button class="close" onclick={onClose} title={t.views.nowPlaying.close}>
      <Icon name="close" size={20} />
    </button>
  </div>

  {#if sheet}
    <!-- Android (#173): lyrics or queue fullscreen, with what it takes to control playback. -->
    <div class="np-sheet">
      <div class="np-sheet-head">
        {#if current}
          {#if current.album.coverUrl}<img src={coverSmall(current.album)} alt="" />{/if}
          <div class="np-sheet-meta">
            <span class="np-sheet-title">{fullTitle(current)}</span>
            <span class="np-sheet-artist">{current.artists.map((a) => a.name).join(", ")}</span>
          </div>
        {/if}
      </div>
      {#if lyricsOpen}
        <section
          class="panel np-panel np-lyrics np-sheet-body"
          aria-label={t.views.nowPlaying.lyrics}
        >
          <Lyrics />
        </section>
      {:else}
        <section class="panel np-panel np-queue np-sheet-body" aria-label={t.player.queue}>
          <QueueList current={false} />
        </section>
      {/if}
      <div class="controls np-sheet-controls">
        <Progress />
        <Transport />
      </div>
    </div>
  {:else}
    <div class="scroller" class:with-panel={anyOpen} class:two-panels={bothOpen}>
      <div class="stage">
        {#if current}
          {@const track = current}
          <div class="layout">
            <div class="art">
              {#if track.album.coverUrl}
                <img src={coverLarge(track.album)} alt="" />
              {/if}
            </div>
            <div class="info">
              <span class="label">{t.views.nowPlaying.title}</span>
              <h1 class="display">{fullTitle(track)}</h1>
              <div class="who">
                {#each track.artists as artist, i (artist.id)}
                  {#if i > 0},&nbsp;{/if}<button
                    class="link"
                    onclick={() => go({ name: "artist", id: artist.id })}
                  >
                    {artist.name}
                  </button>
                {/each}
              </div>
              <button class="link album" onclick={() => go({ name: "album", id: track.album.id })}>
                {track.album.title}
              </button>
              {#if edition && stream}
                <!-- Stream taken from another hi-res edition (#18, #111): the track stays the one
                     chosen (like, playlists, queue), only the sound's source is shown. -->
                <div class="played-from played-edition" title={t.player.hiresEditionHint}>
                  <span class="played-from-label">{t.views.nowPlaying.playedEdition}</span>
                  <button
                    class="link played-from-link"
                    onclick={() => go({ name: "album", id: edition.id })}
                  >
                    {edition.title}
                  </button>
                  <span class="played-edition-format">· {formatStream(stream)}</span>
                </div>
              {/if}
              {#if from}
                <div class="played-from">
                  <span class="played-from-label">{t.views.nowPlaying.playedFrom}</span>
                  <button class="link played-from-link" onclick={() => go(from.route)}>
                    {from.label}
                  </button>
                </div>
              {/if}
              <div class="now-actions">
                <LikeButton {track} size={20} />
                <button
                  class="now-add"
                  onclick={() =>
                    openPlaylistPicker({ trackIds: [track.id], label: fullTitle(track) })}
                  title={t.views.header.addToPlaylist}
                  aria-label={t.views.header.addToPlaylist}
                >
                  <Icon name="playlistAdd" size={20} />
                </button>
                <button
                  class="now-add"
                  onclick={() => go({ name: "radio", seed: "track", id: track.id })}
                  title={t.menu.trackRadio}
                  aria-label={t.menu.trackRadio}
                >
                  <Icon name="radio" size={20} />
                </button>
                <button
                  class="now-add"
                  onclick={() => openTrackInfo(track)}
                  title={t.menu.trackInfo}
                  aria-label={t.menu.trackInfo}
                >
                  <Icon name="info" size={20} />
                </button>
                <!-- #334: menu of the current track (blocking included); blocking skips to the next. -->
                <button
                  class="now-add"
                  bind:this={moreButton}
                  onclick={() => toggleMore(track)}
                  title={t.views.row.more}
                  aria-label={t.views.row.more}
                  aria-haspopup="menu"
                >
                  <Icon name="more" size={20} />
                </button>
                <!-- #25: quick access to the Jam, lit while it runs. -->
                {#if jamAvailable(!platform.mobile)}
                  <button
                    class="now-add jam-open"
                    class:active={!!jam.status}
                    onclick={openJamDialog}
                    title={jam.status ? t.jam.openRunning : t.jam.open}
                    aria-label={jam.status ? t.jam.openRunning : t.jam.open}
                  >
                    <Icon name="jam" size={20} />
                  </button>
                {/if}
                {#if platform.mobile && handoff.linked}
                  <button
                    class="now-add handoff-open"
                    bind:this={handoffButton}
                    disabled={handoff.busy}
                    onclick={toggleHandoff}
                    title={t.handoff.menu}
                    aria-label={t.handoff.menu}
                    aria-haspopup="menu"
                  >
                    <Icon name="devices" size={20} />
                  </button>
                {/if}
                <SleepTimer size={20} />
              </div>

              <div class="controls">
                <Progress large />
                <Transport large />
                <Volume />
              </div>

              <div class="signal"><SignalPath /></div>

              {#if upNext.length > 0 && !anyOpen}
                <div class="next">
                  <span class="label">{t.views.nowPlaying.upNext}</span>
                  <ol>
                    {#each upNext as next, i (`${i}-${next.id}`)}
                      <li>
                        <button
                          class="next-item"
                          onclick={() => playUpNext(i, next)}
                          aria-label={t.views.row.play(fullTitle(next))}
                        >
                          {#if next.album.coverUrl}<img src={coverSmall(next.album)} alt="" />{/if}
                          <span class="t">{fullTitle(next)}</span>
                          <span class="a">{next.artists.map((a) => a.name).join(", ")}</span>
                        </button>
                      </li>
                    {/each}
                  </ol>
                </div>
              {/if}
            </div>
          </div>
        {:else}
          <p class="empty">{t.views.nowPlaying.empty}</p>
        {/if}
      </div>

      {#if lyricsOpen}
        <aside class="panel np-panel np-lyrics" aria-label={t.views.nowPlaying.lyrics}>
          <h2>{t.views.nowPlaying.lyrics}</h2>
          <Lyrics />
          {#if !queueOpen && upNext.length > 0}
            <!-- Without the queue beside them, the next tracks stay visible under the lyrics (#119). -->
            <section class="np-next" aria-label={t.views.nowPlaying.upNext}>
              <div class="np-next-head">
                <h3>{t.views.nowPlaying.upNext}</h3>
                <button class="np-next-all" onclick={() => toggle("queue")}>
                  {t.views.nowPlaying.fullQueue}
                </button>
              </div>
              <div role="list">
                {#each upNext.slice(0, 3) as next, i (`${i}-${next.id}`)}
                  <TrackRow
                    track={next}
                    compact
                    actions={false}
                    onPlay={() => playUpNext(i, next)}
                  />
                {/each}
              </div>
            </section>
          {/if}
        </aside>
      {/if}
      {#if queueOpen}
        <aside class="panel np-panel np-queue" aria-label={t.player.queue}>
          <h2>{ui.settings.theme === "hotel" ? t.player.queueHotel : t.player.queue}</h2>
          <QueueList current={false} />
        </aside>
      {/if}
    </div>
  {/if}
</div>

<style>
  /* Widescreen factor (#11): 0 without bars, 1 with them. Registered so it can be transitioned:
     bars and inset follow the same value, and a window resize changes `--np-bars` at once
     instead of lagging behind. */
  @property --np-scope {
    syntax: "<number>";
    inherits: true;
    initial-value: 0;
  }

  .now-playing {
    --np-panel-width: 380px;
    /* Aim for a 2.39:1 picture between the bars, capped at 10vh and never leaving less than
       560 px of content; none in the single-column layout (max-width: 900px below). */
    --np-bars: max(0px, min((100vh - 100vw / 2.39) / 2, (100vh - 560px) / 2, 10vh));
    /* Inset of the content: the larger of the theme's own bars (Showing) and the easter egg's. */
    --np-inset: max(var(--np-theme-bars, 0px), var(--np-bars) * var(--np-scope));
    --np-scope: 0;
    transition: --np-scope 1.2s ease-out;
    position: fixed;
    inset: 0;
    z-index: 50;
    overflow: hidden;
    background: var(--bg);
    animation: rise 0.35s cubic-bezier(0.2, 0.8, 0.2, 1);
  }

  .now-playing.scope {
    --np-scope: 1;
  }

  :global(:root[data-motion="reduced"]) .now-playing {
    transition: none;
  }

  /* Widescreen (#399): two black bars slide in at the top and bottom, and out when the next
     track is not a soundtrack. The content is inset by as much (`.scroller`, #11). */
  .egg-scope::before,
  .egg-scope::after {
    content: "";
    position: absolute;
    left: 0;
    right: 0;
    /* Above the layout (1), under the top buttons (2, placed after). */
    z-index: 2;
    height: var(--np-bars);
    background: #000;
    pointer-events: none;
  }

  .egg-scope::before {
    top: 0;
    translate: 0 calc((var(--np-scope) - 1) * 100%);
  }

  .egg-scope::after {
    bottom: 0;
    translate: 0 calc((1 - var(--np-scope)) * 100%);
  }

  /* Android: the status bar and the navigation already hold the edges. */
  :global(:root[data-platform="android"]) .now-playing {
    --np-bars: 0px;
  }

  :global(:root[data-platform="android"]) .egg-scope {
    display: none;
  }

  .now-playing.lyrics-open {
    --np-panel-width: 460px;
  }

  /* Lyrics + queue (#119): both columns and the gap between them. */
  .now-playing.both-open {
    --np-panel-width: calc(460px + 16px + 380px);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(24px) scale(0.98);
    }
  }

  .top-actions {
    position: absolute;
    top: 18px;
    right: 18px;
    z-index: 2;
    display: flex;
    gap: 8px;
  }

  .close,
  .fullscreen-toggle,
  .queue-toggle,
  .lyrics-toggle {
    display: grid;
    place-items: center;
    width: 40px;
    height: 40px;
    border-radius: 50%;
    color: var(--text);
    background: var(--bg-raised);
  }

  .close:hover,
  .fullscreen-toggle:hover,
  .queue-toggle:hover,
  .lyrics-toggle:hover {
    background: var(--bg-hover);
  }

  .queue-toggle.on,
  .lyrics-toggle.on {
    color: var(--accent);
  }

  /* Scrolling: the whole view, or each column when the queue is on the right. Inset between
     the widescreen bars (#11). */
  .scroller {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr);
    height: 100%;
    padding-block: var(--np-inset);
    overflow: auto;
  }

  .stage {
    display: grid;
    min-width: 0;
  }

  .layout {
    position: relative;
    display: grid;
    grid-template-columns: minmax(240px, 46vh) minmax(320px, 560px);
    align-items: center;
    gap: clamp(32px, 6vw, 88px);
    margin: auto;
    padding: 48px;
  }

  .art {
    aspect-ratio: 1;
    border-radius: var(--radius-cover, 14px);
    overflow: hidden;
    background: var(--bg-raised);
    box-shadow: var(
      --shadow-cover,
      0 30px 80px rgb(0 0 0 / 0.5),
      0 0 120px color-mix(in oklab, var(--album) 35%, transparent)
    );
  }

  .art img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .info {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  h1 {
    margin: 0;
    font-size: clamp(28px, 4.2vw, 52px);
    line-height: 1.05;
  }

  .now-actions {
    display: flex;
    gap: 6px;
    margin-left: -4px;
  }

  .now-add {
    display: grid;
    place-items: center;
    padding: 4px;
    border-radius: 4px;
    color: var(--text-muted);
  }

  .jam-open.active {
    color: var(--accent);
  }

  .now-add:hover {
    color: var(--text);
  }

  .who {
    font-size: 16px;
    color: var(--text-muted);
  }

  .link {
    color: var(--text);
  }

  .link.album {
    align-self: flex-start;
    text-align: left;
    color: var(--text-muted);
  }

  .link:hover {
    text-decoration: underline;
  }

  /* "Played from" (#124): discreet label, clickable name. */
  .played-from {
    display: flex;
    align-items: baseline;
    gap: 6px;
    min-width: 0;
    font-size: 13px;
  }

  .played-from-label {
    flex: none;
    color: var(--text-muted);
  }

  .played-from-link {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .played-edition-format {
    flex: none;
    color: var(--text-muted);
    font-variant-numeric: tabular-nums;
  }

  .controls {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 18px;
    margin: 22px 0 8px;
  }

  .signal {
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: 12px;
    background: var(--bg-raised);
    backdrop-filter: blur(12px);
  }

  .next {
    margin-top: 10px;
  }

  ol {
    margin: 8px 0 0;
    padding: 0;
    list-style: none;
  }

  /* An "Up next" track plays in one click (#167). */
  .next-item {
    display: grid;
    grid-template-columns: 32px minmax(0, 1fr) minmax(0, 0.8fr);
    align-items: center;
    gap: 10px;
    width: 100%;
    margin: 0 -6px;
    padding: 5px 6px;
    box-sizing: content-box;
    border-radius: 6px;
    font: inherit;
    font-size: 13px;
    color: inherit;
    text-align: left;
    background: none;
  }

  .next-item:hover {
    background: var(--bg-hover);
  }

  li img {
    width: 32px;
    height: 32px;
    border-radius: 3px;
  }

  .t,
  .a {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .a {
    color: var(--text-muted);
  }

  .empty {
    margin: auto;
    color: var(--text-muted);
  }

  /* Easter egg (#399), Cyberpunk theme: nothing playing, the screen shows fixed TV snow
     (SVG noise, no animation). */
  :global(:root[data-eggs][data-theme="cyberpunk"]) .empty {
    display: grid;
    place-items: center;
    width: min(100%, 360px);
    aspect-ratio: 4 / 3;
    padding: 16px;
    border: 1px solid var(--border);
    background:
      linear-gradient(rgb(0 0 0 / 0.35), rgb(0 0 0 / 0.35)),
      url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='160' height='120'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='1.4' numOctaves='2' stitchTiles='stitch'/%3E%3CfeColorMatrix type='saturate' values='0'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E");
    color: #e6f7ff;
    text-shadow: 0 0 6px #000;
  }

  /* Side panel: under the view by default (narrow window), on the right when there is room. */
  .np-panel {
    position: relative;
    justify-self: center;
    width: min(100% - 48px, 420px);
    margin-bottom: 24px;
    padding: 16px 12px;
  }

  .np-lyrics {
    display: flex;
    flex-direction: column;
    height: min(70vh, 640px);
  }

  h2 {
    margin: 0 8px 4px;
    font-size: 15px;
    font-weight: 600;
  }

  /* "Up next" strip under the lyrics (#119). */
  .np-next {
    flex: none;
    margin-top: 8px;
    padding-top: 10px;
    border-top: 1px solid var(--border);
  }

  .np-next-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    margin: 0 8px 4px;
  }

  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .np-next-all {
    font-size: 12px;
    color: var(--text-muted);
  }

  .np-next-all:hover {
    color: var(--text);
    text-decoration: underline;
  }

  @media (max-width: 900px) {
    .now-playing {
      --np-bars: 0px;
    }

    .layout {
      grid-template-columns: minmax(0, 420px);
      padding: 72px 24px 24px;
    }
  }

  @media (min-width: 901px) {
    .scroller.with-panel {
      grid-template-columns: minmax(0, 1fr) var(--np-panel-width);
      grid-template-rows: minmax(0, 1fr);
      overflow: hidden;
    }

    .scroller.two-panels {
      grid-template-columns: minmax(0, 1fr) 460px 380px;
    }

    .with-panel .stage {
      min-height: 0;
      overflow: auto;
    }

    .with-panel .np-panel {
      justify-self: stretch;
      width: auto;
      height: auto;
      margin: 16px 16px 16px 0;
      overflow: auto;
    }

    /* The lyrics scroll in their own area, under the panel's title. */
    .with-panel .np-lyrics {
      overflow: hidden;
    }

    /* The buttons stay above the cover, not on the panel. */
    .top-actions.with-panel {
      right: calc(var(--np-panel-width) + 34px);
    }

    .with-panel .layout {
      grid-template-columns: minmax(200px, min(46vh, 32vw)) minmax(320px, 560px);
      gap: clamp(32px, 4vw, 64px);
    }
  }

  /* In between: queue on the right, cover above the controls. */
  @media (min-width: 901px) and (max-width: 1280px) {
    .with-panel .layout {
      grid-template-columns: minmax(0, 420px);
      padding: 72px 32px 32px;
    }
  }

  /* Two panels: cover above the controls as long as there is no room beside them. */
  @media (max-width: 1799px) {
    .two-panels .layout {
      grid-template-columns: minmax(0, 420px);
      padding: 72px 32px 32px;
    }
  }

  /* Studio: no blur, sharp grid, cover without a colored shadow. */
  :global([data-theme="studio"]) .art {
    box-shadow: none;
    outline: 1px solid var(--border);
  }

  :global([data-theme="studio"]) .signal {
    border-radius: 0;
    backdrop-filter: none;
  }

  /* Android (#146): no blur recomputed every frame under the animated signal path. */
  :global(:root[data-platform="android"]) .signal {
    backdrop-filter: none;
  }

  :global([data-theme="studio"]) .close,
  :global([data-theme="studio"]) .fullscreen-toggle,
  :global([data-theme="studio"]) .queue-toggle,
  :global([data-theme="studio"]) .lyrics-toggle {
    border-radius: 0;
  }

  :global([data-theme="studio"]) .np-panel,
  :global([data-theme="atelier"]) .np-panel {
    border: 1px solid var(--border);
  }

  /* Atelier: magazine page, no blurred background. */
  :global([data-theme="atelier"]) .art {
    box-shadow: 18px 18px 0 color-mix(in oklab, var(--album) 35%, var(--bg));
  }

  :global([data-theme="atelier"]) .signal {
    border-radius: 0;
    border-color: var(--text);
    background: transparent;
  }

  :global([data-theme="atelier"]) h1 {
    font-size: clamp(32px, 5vw, 64px);
  }

  /* Android (#24): fullscreen under the system bars; the volume goes through the phone's
     keys. */
  :global(:root[data-platform="android"]) .top-actions {
    top: calc(var(--safe-top) + 8px);
    right: 12px;
  }

  :global(:root[data-platform="android"]) .layout {
    gap: 16px;
    padding: calc(var(--safe-top) + 56px) 20px calc(var(--safe-bottom) + 24px);
  }

  /* #195: everything down to play / pause fits above the phone's buttons. No "Now playing"
     label (the whole screen says it), cover height bounded on short screens, album and
     "Played from" on one line, tighter controls. */
  :global(:root[data-platform="android"]) .art {
    width: min(100%, 40dvh);
    justify-self: center;
  }

  :global(:root[data-platform="android"]) .info {
    gap: 8px;
  }

  :global(:root[data-platform="android"]) .info > .label {
    display: none;
  }

  :global(:root[data-platform="android"]) .link.album {
    max-width: 100%;
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  :global(:root[data-platform="android"]) .controls {
    gap: 12px;
    margin: 12px 0 8px;
  }

  :global(:root[data-platform="android"]) .controls :global(.volume) {
    display: none;
  }

  :global(:root[data-platform="android"]) .np-panel {
    width: calc(100% - 24px);
    margin-bottom: calc(var(--safe-bottom) + 16px);
  }

  /* Android (#173): lyrics or queue fullscreen. Small header next to the top buttons, panel
     over the whole remaining height, progress and controls at the bottom. */
  .np-sheet {
    position: relative;
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    padding: calc(var(--safe-top) + 8px) 12px calc(var(--safe-bottom) + 12px);
  }

  .np-sheet-head {
    display: flex;
    flex: none;
    align-items: center;
    gap: 10px;
    min-height: 40px;
    /* Room for the three top buttons (Lyrics, Queue, Close). */
    padding-right: 140px;
  }

  .np-sheet-head img {
    flex: none;
    width: 40px;
    height: 40px;
    border-radius: var(--radius-cover, 6px);
    object-fit: cover;
  }

  .np-sheet-meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .np-sheet-title,
  .np-sheet-artist {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .np-sheet-title {
    font-size: 14px;
    font-weight: 600;
  }

  .np-sheet-artist {
    font-size: 12px;
    color: var(--text-muted);
  }

  /* Android prefix: without it, Android's `.np-panel` rule (more specific) wins on the width
     and the bottom margin, and the tile is shifted to the left (#242). */
  :global(:root[data-platform="android"]) .np-sheet .np-sheet-body {
    display: flex;
    flex: 1;
    flex-direction: column;
    width: auto;
    height: auto;
    min-height: 0;
    margin: 0;
    padding: 12px 8px;
    overflow: hidden;
  }

  /* Same prefix as the rule above, otherwise its `overflow: hidden` wins and the queue no
     longer scrolls (#342). */
  :global(:root[data-platform="android"]) .np-sheet .np-queue {
    overflow: auto;
  }

  .np-sheet-controls {
    flex: none;
    gap: 10px;
    margin: 0;
  }
</style>
