<script lang="ts">
  // Mode conduite (#193, Android) : plein écran, titre en cours, gros boutons de lecture et
  // tuiles des épingles (8 au plus). Un appui sur une tuile lit toute la source tout de suite ;
  // appui long : avancer, reculer, retirer. L'écran reste allumé tant que la vue est ouverte.
  import { onMount } from "svelte";
  import { commands, type Pin, type PlaySource } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import { queue } from "../lib/stores/queue.svelte";
  import { MAX_DRIVE_TILES, movePin, pinKey, togglePin, ui } from "../lib/stores/ui.svelte";
  import { openMenu } from "../lib/stores/menu.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { coverSmall, fullTitle } from "../lib/format";
  import { pinLabel } from "../lib/pins";
  import { t } from "../lib/i18n.svelte";
  import Icon, { type IconName } from "../components/Icon.svelte";

  let { onClose }: { onClose: () => void } = $props();

  const s = $derived(player.state);
  const track = $derived(player.currentTrack);
  const playing = $derived(!s.paused && !s.idle);
  const tiles = $derived(ui.settings.pins.slice(0, MAX_DRIVE_TILES));
  /** Tuile dont la lecture se prépare (titres cherchés par le backend). */
  let starting = $state<string | null>(null);

  onMount(() => {
    void commands.keepScreenOn(true).catch(() => {});
    return () => void commands.keepScreenOn(false).catch(() => {});
  });

  const ICONS: Record<Pin["kind"], IconName> = {
    playlist: "library",
    mix: "radio",
    likedTracks: "heartFilled",
    later: "bookmarkFilled",
  };

  /** La tuile est la source du titre en cours. */
  function isCurrent(pin: Pin, source: PlaySource | null): boolean {
    if (!source || source.kind !== pin.kind) return false;
    if (pin.kind === "playlist") return source.kind === "playlist" && source.uuid === pin.uuid;
    if (pin.kind === "mix") return source.kind === "mix" && source.id === pin.id;
    return true;
  }

  async function play(pin: Pin) {
    const key = pinKey(pin);
    if (starting) return;
    starting = key;
    try {
      await commands.playPin(pin, s.shuffle);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    } finally {
      starting = null;
    }
  }

  function edit(e: MouseEvent, pin: Pin, index: number) {
    e.preventDefault();
    const fail = (err: unknown) => pushToast({ level: "error", message: String(err) });
    openMenu({
      x: e.clientX,
      y: e.clientY,
      from: e.currentTarget as HTMLElement,
      entries: [
        ...(index > 0 ? [{ label: t.drive.moveUp, icon: "up" as const, action: () => movePin(pin, -1).catch(fail) }] : []),
        ...(index < tiles.length - 1
          ? [{ label: t.drive.moveDown, icon: "down" as const, action: () => movePin(pin, 1).catch(fail) }]
          : []),
        { label: t.drive.remove, icon: "close", action: () => togglePin(pin).catch(fail) },
      ],
    });
  }
</script>

<div class="drive">
  <div class="drive-top">
    <button class="drive-close" onclick={onClose} aria-label={t.drive.close}>
      <Icon name="close" size={26} />
    </button>
    <span class="drive-title">{t.drive.open}</span>
    <button
      class="drive-shuffle"
      class:on={s.shuffle}
      onclick={() => commands.setShuffle(!s.shuffle)}
      aria-pressed={s.shuffle}
      aria-label={s.shuffle ? t.player.shuffleOn : t.player.shuffleOff}
    >
      <Icon name="shuffle" size={26} />
    </button>
  </div>

  <div class="drive-now">
    <span class="drive-cover">
      {#if track?.album.coverUrl}<img src={coverSmall(track.album)} alt="" />{/if}
    </span>
    <span class="drive-meta">
      <span class="drive-track">{track ? fullTitle(track) : t.drive.nothing}</span>
      {#if track}<span class="drive-artist">{track.artists.map((a) => a.name).join(", ")}</span>{/if}
    </span>
  </div>

  <div class="drive-transport">
    <button onclick={() => commands.previous()} disabled={!track} aria-label={t.player.previous}>
      <Icon name="previous" size={36} />
    </button>
    <button
      class="drive-play"
      onclick={() => commands.togglePause()}
      disabled={!track}
      aria-label={playing ? t.player.pause : t.player.play}
    >
      {#if s.loading || s.buffering}
        <span class="spinner"></span>
      {:else}
        <Icon name={playing ? "pause" : "play"} size={44} />
      {/if}
    </button>
    <button onclick={() => commands.next()} disabled={!s.hasNext} aria-label={t.player.next}>
      <Icon name="next" size={36} />
    </button>
  </div>

  {#if tiles.length === 0}
    <p class="drive-empty">{t.drive.empty}</p>
  {:else}
    <div class="drive-tiles" class:single={tiles.length === 1} style:--rows={Math.ceil(tiles.length / 2)}>
      {#each tiles as pin, i (pinKey(pin))}
        <button
          class="drive-tile"
          class:current={isCurrent(pin, queue.view.source)}
          class:wide={tiles.length % 2 === 1 && i === tiles.length - 1}
          onclick={() => play(pin)}
          oncontextmenu={(e) => edit(e, pin, i)}
          aria-busy={starting === pinKey(pin)}
        >
          {#if starting === pinKey(pin)}
            <span class="spinner"></span>
          {:else}
            <Icon name={ICONS[pin.kind]} size={28} />
          {/if}
          <span class="drive-tile-label">{pinLabel(pin)}</span>
        </button>
      {/each}
    </div>
  {/if}
</div>

<style>
  .drive {
    position: fixed;
    inset: 0;
    z-index: 45;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: calc(var(--safe-top) + 10px) 14px calc(var(--safe-bottom) + 14px);
    background: var(--bg);
    color: var(--text);
  }

  .drive-top {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .drive-close,
  .drive-shuffle {
    display: grid;
    place-items: center;
    width: 56px;
    height: 56px;
    border-radius: 50%;
    color: var(--text-muted);
  }

  .drive-shuffle.on {
    color: var(--accent);
  }

  .drive-title {
    font-family: var(--meta-family);
    font-size: 13px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .drive-now {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .drive-cover {
    flex: none;
    width: 76px;
    height: 76px;
    overflow: hidden;
    border-radius: var(--radius);
    background: var(--bg-raised);
  }

  .drive-cover img {
    width: 100%;
    height: 100%;
    object-fit: cover;
  }

  .drive-meta {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 0;
  }

  .drive-track,
  .drive-artist {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .drive-track {
    font-size: 22px;
    font-weight: 700;
  }

  .drive-artist {
    font-size: 17px;
    color: var(--text-muted);
  }

  .drive-transport {
    display: flex;
    align-items: center;
    justify-content: space-evenly;
  }

  .drive-transport button {
    display: grid;
    place-items: center;
    width: 84px;
    height: 84px;
    border-radius: 50%;
    color: var(--text);
  }

  .drive-transport button:active:not(:disabled) {
    background: var(--bg-hover);
  }

  .drive-transport button:disabled {
    opacity: 0.35;
  }

  .drive-transport .drive-play {
    width: 104px;
    height: 104px;
    color: var(--accent-contrast);
    background: var(--accent);
  }

  .drive-transport .drive-play:active:not(:disabled) {
    background: var(--accent);
    transform: scale(0.95);
  }

  /* Les tuiles se partagent la hauteur restante : 1 tuile = tout l'espace, puis 2 colonnes ;
     une tuile seule sur sa ligne prend la largeur. */
  .drive-tiles {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    grid-template-rows: repeat(var(--rows), minmax(0, 1fr));
    gap: 10px;
    min-height: 0;
  }

  .drive-tiles.single {
    grid-template-columns: minmax(0, 1fr);
  }

  .drive-tile {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    min-height: 0;
    padding: 8px 12px;
    border: 2px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-raised);
    color: var(--text);
    /* Pas de sélection de texte ni de loupe à l'appui long : il ouvre le menu. */
    user-select: none;
    -webkit-user-select: none;
    -webkit-touch-callout: none;
  }

  .drive-tile.wide {
    grid-column: 1 / -1;
  }

  .drive-tile:active {
    background: var(--bg-hover);
  }

  .drive-tile.current {
    border-color: var(--accent);
    color: var(--accent);
  }

  .drive-tile-label {
    display: -webkit-box;
    max-width: 100%;
    overflow: hidden;
    font-size: 19px;
    font-weight: 600;
    line-height: 1.2;
    text-align: center;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .drive-empty {
    margin: auto 8px;
    font-size: 17px;
    line-height: 1.5;
    text-align: center;
    color: var(--text-muted);
  }

  .spinner {
    width: 26px;
    height: 26px;
    border: 3px solid currentColor;
    border-right-color: transparent;
    border-radius: 50%;
    animation: spin 0.8s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }
</style>
