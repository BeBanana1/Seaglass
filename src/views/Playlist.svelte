<script lang="ts">
  import { untrack } from "svelte";
  import { commands, type PlaylistPage, type TrackId } from "../lib/ipc";
  import { formatLongDuration, fullTitle, playlistSubtitle } from "../lib/format";
  import {
    filterTracks,
    listTracks,
    playContext,
    PLAYLIST_DEFAULT_SORT,
    sortTracks,
    type SortKey,
    type TrackSort,
  } from "../lib/trackOrder";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";
  import TrackTools from "../components/TrackTools.svelte";
  import PlaylistEditor from "../components/PlaylistEditor.svelte";
  import CleanupDialog from "../components/CleanupDialog.svelte";
  import {
    isPinned,
    listSort,
    refreshPinTitle,
    saveListSort,
    togglePin,
  } from "../lib/stores/ui.svelte";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import { back } from "../lib/stores/nav.svelte";
  import { notePlaylistChange, playlistChanges } from "../lib/stores/playlistPicker.svelte";
  import { pushToast, pushUndoToast } from "../lib/stores/toasts.svelte";
  import { listSearch } from "../lib/stores/listSearch.svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { ui } from "../lib/stores/ui.svelte";

  /** Easter egg (#399): one empty playlist in ten announces a missing track. Drawn once
   *  per opened page. */
  const missingTrack =
    Math.random() < 0.1 || (import.meta.env.DEV && location.search.includes("egg=404"));

  const SORT_KEYS: SortKey[] = ["position", "added", "title", "artist", "album", "duration"];
  const DEFAULT_SORT = PLAYLIST_DEFAULT_SORT;
  // "Loading…" only if opening takes a while: nothing flickers on a fast page (#152).
  const LOADING_DELAY_MS = 300;

  let { uuid }: { uuid: string } = $props();

  let page = $state<PlaylistPage | null>(null);
  let error = $state<string | null>(null);
  let slow = $state(false);
  let query = $state("");
  let sort = $state<TrackSort>(DEFAULT_SORT);
  let editing = $state(false);
  let cleaning = $state(false);
  let moving = false;

  $effect(() => {
    const target = uuid;
    page = null;
    error = null;
    slow = false;
    query = "";
    editing = false;
    cleaning = false;
    // Remembered sort for this playlist (#93); the filter starts empty.
    sort = untrack(() => listSort(target, DEFAULT_SORT));
    const timer = setTimeout(() => (slow = true), LOADING_DELAY_MS);
    open(target);
    return () => clearTimeout(timer);
  });

  // Tracks added or removed: reload in place (filter and sort kept).
  $effect(() => {
    void playlistChanges.revision;
    if (playlistChanges.uuid === untrack(() => uuid)) untrack(() => load(uuid));
  });

  function setSort(next: TrackSort) {
    sort = next;
    saveListSort(uuid, next).catch((e) => pushToast({ level: "error", message: String(e) }));
  }

  // Copy kept by the backend shown at once (#152), then checked: it is only
  // replaced if the playlist changed since.
  async function open(target: string) {
    const cached = await commands.cachedPlaylist(target).catch(() => null);
    if (target !== uuid) return;
    if (cached && !page) page = cached;
    await load(target);
  }

  async function load(target: string) {
    try {
      const fresh = await commands.playlist(
        target,
        untrack(() => page?.revision ?? null),
      );
      if (target !== uuid || !fresh) return;
      page = fresh;
      // A playlist pinned then renamed: the sidebar picks up the new title.
      refreshPinTitle(target, fresh.playlist.title).catch(() => {});
    } catch (e) {
      if (target !== uuid) return;
      // Kept copy already shown (offline, for example): it stays, the error goes out as a toast.
      if (page) pushToast({ level: "error", message: String(e) });
      else error = String(e);
    }
  }

  // `index`: position in the displayed list; the backend receives the original position.
  async function removeAt(index: number) {
    const entry = shown[index];
    if (!entry) return;
    try {
      await commands.removeFromPlaylist(uuid, entry.position - 1, entry.track.id);
      // Undoable (#349): the track returns to its position, with a new date added.
      const [playlist, position, id] = [uuid, entry.position - 1, entry.track.id];
      pushUndoToast(
        t.views.playlist.removed(fullTitle(entry.track)),
        () => void restore(playlist, position, id),
      );
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    // Reloaded even on failure: a refusal means the playlist changed.
    notePlaylistChange(uuid);
  }

  // Multiple selection (#332): all the displayed positions of the selected tracks, in
  // one request; undoable, each track returns to its position.
  async function removeSelected(ids: ReadonlySet<TrackId>) {
    const playlist = uuid;
    const items = shown
      .filter((l) => ids.has(l.track.id))
      .map((l) => ({ index: l.position - 1, trackId: l.track.id }));
    if (items.length === 0) return;
    try {
      await commands.removeManyFromPlaylist(playlist, items);
      pushUndoToast(t.menu.selection.removed(items.length), async () => {
        try {
          await commands.restoreManyToPlaylist(playlist, items);
        } catch (e) {
          pushToast({ level: "error", message: String(e) });
        }
        notePlaylistChange(playlist);
      });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    notePlaylistChange(playlist);
  }

  async function restore(playlist: string, position: number, id: number) {
    try {
      await commands.restoreToPlaylist(playlist, position, id);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    notePlaylistChange(playlist);
  }

  // Reordering (#201): only on your playlists, in the original order and without a
  // filter (the displayed positions are then those of the playlist). The displayed order changes
  // after confirmation, without rereading the playlist: right after a write, the API
  // sometimes still returns the old order.
  const reorderable = $derived(
    !!page?.playlist.mine && sort.key === "position" && !sort.descending && !query.trim(),
  );

  async function moveTrack(from: number, to: number): Promise<boolean> {
    const current = page;
    const entry = current?.tracks[from];
    // One move at a time: the next one starts from the confirmed positions.
    if (!current || !entry || moving || to < 0 || to >= current.tracks.length) return false;
    moving = true;
    try {
      await commands.moveInPlaylist(uuid, from, to, entry.id);
      if (page !== current) return false;
      const tracks = [...current.tracks];
      tracks.splice(to, 0, ...tracks.splice(from, 1));
      page = { ...current, tracks };
      return true;
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
      // A refusal often means the playlist changed elsewhere: it is reread.
      notePlaylistChange(uuid);
      return false;
    } finally {
      moving = false;
    }
  }

  function saved(title: string, description: string) {
    editing = false;
    if (page)
      page = { ...page, playlist: { ...page.playlist, title, description: description || null } };
    pushToast({ level: "info", message: t.views.playlistEdit.saved });
    refreshPinTitle(uuid, title).catch(() => {});
    if (library.playlists.items) loadLibrary("playlists", true);
  }

  async function deleted() {
    editing = false;
    const pin = { kind: "playlist", uuid, title: page?.playlist.title ?? "" } as const;
    if (isPinned(pin)) await togglePin(pin).catch(() => {});
    if (library.playlists.items) loadLibrary("playlists", true);
    back();
  }

  // Play / Shuffle follow the sort; a click on a track sets the whole sorted list
  // as the context, even filtered (#496), and starts from that track.
  const sorted = $derived(page ? sortTracks(listTracks(page.tracks), sort) : []);
  const shown = $derived(filterTracks(sorted, query));
</script>

<section>
  {#if error}
    <p class="error">{error}</p>
  {:else if !page}
    {#if slow}<p class="muted">{t.common.loading}</p>{/if}
  {:else}
    {@const playlist = page.playlist}
    <!-- Android: header hidden during a search in the playlist (#151). -->
    {#if !listSearch.active}
      <PageHeader
        image={playlist.imageUrl}
        eyebrow={t.format.playlist}
        title={playlist.title}
        description={playlist.description}
        trackIds={sorted.map((l) => l.track.id)}
        source={{ kind: "playlist", uuid: playlist.uuid, title: playlist.title }}
        queueable
        pin={{ kind: "playlist", uuid: playlist.uuid, title: playlist.title }}
        favorite={playlist.mine ? undefined : { kind: "playlist", uuid: playlist.uuid }}
        onEdit={playlist.mine ? () => (editing = true) : undefined}
      >
        {#snippet meta()}
          {playlistSubtitle(playlist)}
          {#if formatLongDuration(playlist.duration)}
            · {formatLongDuration(playlist.duration)}{/if}
        {/snippet}
      </PageHeader>
    {/if}
    {#if page.tracks.length > 1}
      <TrackTools
        bind:query
        bind:sort={() => sort, setSort}
        keys={SORT_KEYS}
        shown={shown.length}
        total={page.tracks.length}
      />
    {/if}
    <!-- Android: no numbers, they clutter more than they help (#162). -->
    {#if shown.length === 0 && query.trim()}
      <p class="muted empty">{t.library.noMatch(query.trim())}</p>
    {:else if page.tracks.length === 0}
      <p class="muted empty">
        {missingTrack && ui.settings.easterEggs ? t.eggs.track404 : t.library.empty}
      </p>
    {:else}
      <TrackList
        tracks={shown.map((l) => l.track)}
        positions={shown.map((l) => l.position)}
        context={(i) => playContext(sorted, shown, i)}
        source={{ kind: "playlist", uuid: page.playlist.uuid, title: page.playlist.title }}
        numbering={platform.mobile ? "none" : "index"}
        onRemoveFromPlaylist={page.playlist.mine ? removeAt : undefined}
        onRemoveSelected={page.playlist.mine ? removeSelected : undefined}
        onMove={reorderable ? moveTrack : undefined}
      />
    {/if}
    {#if editing}
      <PlaylistEditor
        playlist={page.playlist}
        onClose={() => (editing = false)}
        onSaved={saved}
        onDeleted={deleted}
        onCleanup={() => {
          editing = false;
          cleaning = true;
        }}
      />
    {/if}
    {#if cleaning}
      {@const target = page.playlist.uuid}
      <CleanupDialog
        title={t.views.cleanup.playlistTitle}
        positions
        load={() => commands.playlistCleanup(target)}
        apply={(ops) => commands.cleanPlaylist(target, ops)}
        undo={(ops) => commands.undoPlaylistCleanup(target, ops)}
        onClose={() => (cleaning = false)}
        onChanged={() => notePlaylistChange(target)}
      />
    {/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  .muted {
    color: var(--text-muted);
  }

  .empty {
    margin: 0 10px;
  }

  .error {
    color: var(--danger);
  }
</style>
