<script lang="ts">
  import { untrack } from "svelte";
  import { commands, type PlaylistPage } from "../lib/ipc";
  import { formatLongDuration, fullTitle, playlistSubtitle } from "../lib/format";
  import {
    filterTracks,
    listTracks,
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
  import { isPinned, listSort, refreshPinTitle, saveListSort, togglePin } from "../lib/stores/ui.svelte";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import { back } from "../lib/stores/nav.svelte";
  import { notePlaylistChange, playlistChanges } from "../lib/stores/playlistPicker.svelte";
  import { pushToast, pushUndoToast } from "../lib/stores/toasts.svelte";
  import { listSearch } from "../lib/stores/listSearch.svelte";
  import { t } from "../lib/i18n.svelte";
  import { platform } from "../lib/platform.svelte";
  import { ui } from "../lib/stores/ui.svelte";

  /** Clin d'œil (#399) : une playlist vide sur dix annonce une piste introuvable. Tiré une
   *  fois par page ouverte. */
  const missingTrack = Math.random() < 0.1 || (import.meta.env.DEV && location.search.includes("egg=404"));

  const SORT_KEYS: SortKey[] = ["position", "added", "title", "artist", "album", "duration"];
  const DEFAULT_SORT = PLAYLIST_DEFAULT_SORT;
  // « Chargement… » seulement si l'ouverture dure : rien ne clignote sur une page rapide (#152).
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
    // Tri mémorisé pour cette playlist (#93) ; le filtre repart vide.
    sort = untrack(() => listSort(target, DEFAULT_SORT));
    const timer = setTimeout(() => (slow = true), LOADING_DELAY_MS);
    open(target);
    return () => clearTimeout(timer);
  });

  // Titres ajoutés ou retirés : rechargement sur place (filtre et tri gardés).
  $effect(() => {
    void playlistChanges.revision;
    if (playlistChanges.uuid === untrack(() => uuid)) untrack(() => load(uuid));
  });

  function setSort(next: TrackSort) {
    sort = next;
    saveListSort(uuid, next).catch((e) => pushToast({ level: "error", message: String(e) }));
  }

  // Copie gardée par le backend affichée aussitôt (#152), puis vérifiée : elle n'est
  // remplacée que si la playlist a changé depuis.
  async function open(target: string) {
    const cached = await commands.cachedPlaylist(target).catch(() => null);
    if (target !== uuid) return;
    if (cached && !page) page = cached;
    await load(target);
  }

  async function load(target: string) {
    try {
      const fresh = await commands.playlist(target, untrack(() => page?.revision ?? null));
      if (target !== uuid || !fresh) return;
      page = fresh;
      // Une playlist épinglée puis renommée : la barre latérale reprend le nouveau titre.
      refreshPinTitle(target, fresh.playlist.title).catch(() => {});
    } catch (e) {
      if (target !== uuid) return;
      // Copie gardée déjà affichée (hors ligne, par exemple) : elle reste, l'erreur part en toast.
      if (page) pushToast({ level: "error", message: String(e) });
      else error = String(e);
    }
  }

  // `index` : position dans la liste affichée ; le backend reçoit la position d'origine.
  async function removeAt(index: number) {
    const entry = shown[index];
    if (!entry) return;
    try {
      await commands.removeFromPlaylist(uuid, entry.position - 1, entry.track.id);
      // Annulable (#349) : le titre revient à sa position, avec une nouvelle date d'ajout.
      const [playlist, position, id] = [uuid, entry.position - 1, entry.track.id];
      pushUndoToast(t.views.playlist.removed(fullTitle(entry.track)), () => void restore(playlist, position, id));
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    // Rechargée même en cas d'échec : un refus signifie que la playlist a changé.
    notePlaylistChange(uuid);
  }

  async function restore(playlist: string, position: number, id: number) {
    try {
      await commands.restoreToPlaylist(playlist, position, id);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
    notePlaylistChange(playlist);
  }

  // Réorganisation (#201) : seulement sur vos playlists, dans l'ordre d'origine et sans
  // filtre (les positions affichées sont alors celles de la playlist). L'ordre affiché change
  // après confirmation, sans relire la playlist : juste après une écriture, l'API renvoie
  // parfois encore l'ancien ordre.
  const reorderable = $derived(
    !!page?.playlist.mine && sort.key === "position" && !sort.descending && !query.trim(),
  );

  async function moveTrack(from: number, to: number): Promise<boolean> {
    const current = page;
    const entry = current?.tracks[from];
    // Un déplacement à la fois : le suivant part des positions confirmées.
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
      // Un refus signifie souvent que la playlist a changé ailleurs : elle est relue.
      notePlaylistChange(uuid);
      return false;
    } finally {
      moving = false;
    }
  }

  function saved(title: string, description: string) {
    editing = false;
    if (page) page = { ...page, playlist: { ...page.playlist, title, description: description || null } };
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

  // Lecture / Aléatoire suivent le tri ; un clic sur un titre met en contexte la liste
  // affichée (triée et filtrée), dans cet ordre.
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
    <!-- Android : en-tête masqué pendant une recherche dans la playlist (#151). -->
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
          {#if formatLongDuration(playlist.duration)} · {formatLongDuration(playlist.duration)}{/if}
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
    <!-- Android : pas de numéros, ils encombrent plus qu'ils n'aident (#162). -->
    {#if shown.length === 0 && query.trim()}
      <p class="muted empty">{t.library.noMatch(query.trim())}</p>
    {:else if page.tracks.length === 0}
      <p class="muted empty">{missingTrack && ui.settings.easterEggs ? t.eggs.track404 : t.library.empty}</p>
    {:else}
      <TrackList
        tracks={shown.map((l) => l.track)}
        positions={shown.map((l) => l.position)}
        source={{ kind: "playlist", uuid: page.playlist.uuid, title: page.playlist.title }}
        numbering={platform.mobile ? "none" : "index"}
        onRemoveFromPlaylist={page.playlist.mine ? removeAt : undefined}
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
