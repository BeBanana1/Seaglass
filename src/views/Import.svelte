<script lang="ts">
  // Import d'une playlist d'un autre service (#328) : le backend lit la playlist et cherche
  // chaque titre sur TIDAL (`importPlaylist`), la page montre les titres trouvés, ceux à
  // vérifier (décochés tant que l'utilisateur ne les garde pas) et les introuvables. Rien
  // n'est enregistré avant « Créer une playlist ».
  import { onDestroy } from "svelte";
  import { commands, events, type ImportPreview, type PlaySource, type Track } from "../lib/ipc";
  import { formatLongDuration, formatTime } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { go } from "../lib/stores/nav.svelte";
  import { library, loadLibrary } from "../lib/stores/library.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import { platform } from "../lib/platform.svelte";
  import PageHeader from "../components/PageHeader.svelte";
  import TrackList from "../components/TrackList.svelte";

  let { link }: { link: string } = $props();

  let preview = $state<ImportPreview | null>(null);
  let error = $state<string | null>(null);
  let progress = $state<{ done: number; total: number } | null>(null);
  /** Positions des titres à vérifier que l'utilisateur garde. */
  let kept = $state<Set<number>>(new Set());
  let creating = $state(false);
  let finished = false;

  async function load() {
    preview = null;
    error = null;
    progress = null;
    kept = new Set();
    finished = false;
    const unlisten = await events.onImportProgress((p) => (progress = p));
    try {
      preview = await commands.importPlaylist(link);
    } catch (e) {
      error = String(e);
    } finally {
      finished = true;
      unlisten();
    }
  }

  void load();

  // Page quittée pendant la recherche : inutile de continuer.
  onDestroy(() => {
    if (!finished) void commands.cancelImport().catch(() => {});
  });

  const items = $derived(preview?.items ?? []);
  const found = $derived(items.filter((i) => i.sure && i.track).map((i) => i.track as Track));
  const toCheck = $derived(
    items.map((item, index) => ({ item, index })).filter(({ item }) => !item.sure && item.track),
  );
  const missing = $derived(items.filter((i) => !i.track));
  /** Titres retenus, dans l'ordre de la playlist d'origine. */
  const keptTracks = $derived(
    items.flatMap((item, index) =>
      item.track && (item.sure || kept.has(index)) ? [item.track] : [],
    ),
  );
  const keptIds = $derived(keptTracks.map((track) => track.id));
  const duration = $derived(keptTracks.reduce((sum, track) => sum + track.duration, 0));
  const source = $derived<PlaySource | undefined>(
    preview ? { kind: "import", link, title: preview.name } : undefined,
  );

  function toggle(index: number) {
    const next = new Set(kept);
    if (next.has(index)) next.delete(index);
    else next.add(index);
    kept = next;
  }

  async function create() {
    if (!preview || creating || keptIds.length === 0) return;
    creating = true;
    try {
      const uuid = await commands.createPlaylist(preview.name, keptIds);
      pushToast({ level: "info", message: t.views.import.created(preview.name) });
      if (library.playlists.items) loadLibrary("playlists", true);
      go({ name: "playlist", uuid });
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    } finally {
      creating = false;
    }
  }

  const artists = (track: Track) => track.artists.map((a) => a.name).join(", ");
</script>

<section class="import">
  {#if error}
    <p class="error">{error}</p>
    <button class="import-retry" onclick={() => void load()}>{t.views.import.retry}</button>
  {:else if !preview}
    <div class="import-loading">
      <p class="muted">
        {progress ? t.views.import.matching(progress.done, progress.total) : t.views.import.reading}
      </p>
      <progress class="import-progress" max={progress?.total || 1} value={progress?.done ?? 0}
      ></progress>
    </div>
  {:else}
    <PageHeader
      image={keptTracks[0]?.album.coverLargeUrl ?? keptTracks[0]?.album.coverUrl ?? null}
      eyebrow={t.views.import.eyebrow}
      title={preview.name}
      description={t.views.import.from(t.views.import.services[preview.service])}
      trackIds={keptIds}
      {source}
      queueable
    >
      {#snippet meta()}
        {t.views.import.kept(keptIds.length)}
        {#if formatLongDuration(duration)}
          · {formatLongDuration(duration)}{/if}
      {/snippet}
    </PageHeader>

    <div class="import-actions">
      <button class="import-create" onclick={create} disabled={creating || keptIds.length === 0}>
        {creating ? t.views.import.creating : t.views.import.create}
      </button>
      {#if preview.read < preview.total}
        <p class="import-partial">{t.views.import.partial(preview.read, preview.total)}</p>
      {/if}
    </div>

    {#if keptIds.length === 0 && toCheck.length === 0}
      <p class="muted">{t.views.import.nothingKept}</p>
    {/if}

    {#if found.length > 0}
      <h2>{t.views.import.found} <span class="count">{found.length}</span></h2>
      <TrackList tracks={found} {source} numbering={platform.mobile ? "none" : "index"} />
    {/if}

    {#if toCheck.length > 0}
      <h2>{t.views.import.toCheck} <span class="count">{toCheck.length}</span></h2>
      <p class="muted hint">{t.views.import.toCheckHint}</p>
      <ul class="import-check">
        {#each toCheck as { item, index } (index)}
          {@const track = item.track as Track}
          <li>
            <label class="import-check-row" class:kept={kept.has(index)}>
              <input type="checkbox" checked={kept.has(index)} onchange={() => toggle(index)} />
              {#if track.album.coverSmallUrl ?? track.album.coverUrl}
                <img src={track.album.coverSmallUrl ?? track.album.coverUrl} alt="" />
              {/if}
              <span class="import-candidate">
                <span class="title">{track.title}{track.version ? ` (${track.version})` : ""}</span>
                <span class="sub">
                  {artists(track)} · {track.album.title} · {formatTime(track.duration)}
                </span>
                <span class="sub original">
                  {t.views.import.originally} : {item.title}{item.artist ? ` · ${item.artist}` : ""}
                </span>
              </span>
            </label>
          </li>
        {/each}
      </ul>
    {/if}

    {#if missing.length > 0}
      <h2>{t.views.import.missing} <span class="count">{missing.length}</span></h2>
      <ul class="import-missing">
        {#each missing as item, i (i)}
          <li>
            <span class="title">{item.title}</span>
            {#if item.artist}<span class="sub">· {item.artist}</span>{/if}
          </li>
        {/each}
      </ul>
    {/if}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  h2 {
    margin: 28px 0 10px;
    font-size: 1.1rem;
  }

  .count {
    margin-left: 6px;
    font-weight: 400;
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
  }

  .hint {
    margin: -4px 0 10px;
    font-size: 0.85rem;
  }

  .error {
    color: var(--danger);
  }

  .import-loading {
    max-width: 420px;
    margin: 48px auto;
    text-align: center;
  }

  .import-progress {
    width: 100%;
    accent-color: var(--accent);
  }

  .import-actions {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 12px 16px;
    margin: -4px 0 8px;
  }

  .import-create {
    padding: 8px 18px;
    border: none;
    border-radius: 999px;
    background: var(--accent);
    color: var(--accent-contrast);
    font: inherit;
    font-weight: 600;
    cursor: pointer;
  }

  .import-create:disabled {
    opacity: 0.5;
    cursor: default;
  }

  .import-partial {
    margin: 0;
    font-size: 0.85rem;
    color: var(--warn);
  }

  .import-retry {
    margin-top: 8px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
  }

  .import-check-row {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px;
    border-radius: 8px;
    cursor: pointer;
    opacity: 0.75;
  }

  .import-check-row.kept {
    opacity: 1;
  }

  .import-check-row:hover {
    background: var(--bg-hover);
  }

  .import-check-row img {
    width: 40px;
    height: 40px;
    border-radius: 4px;
    object-fit: cover;
  }

  .import-candidate {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .title {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sub {
    font-size: 0.85rem;
    color: var(--text-muted);
  }

  .original {
    font-style: italic;
  }

  .import-missing li {
    padding: 6px 8px;
  }
</style>
