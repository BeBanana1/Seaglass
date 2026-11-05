<script lang="ts">
  // Crédits de l'album entier (#206), sous la liste des titres. Demandés au premier clic
  // seulement (une requête par page de 100 titres), puis repliables.
  import { commands, type AlbumCreditGroup, type Track } from "../lib/ipc";
  import { creditRole } from "../lib/format";
  import { t } from "../lib/i18n.svelte";
  import { go } from "../lib/stores/nav.svelte";

  let { albumId, tracks }: { albumId: number; tracks: Track[] } = $props();

  let open = $state(false);
  let credits = $state<AlbumCreditGroup[] | null>(null);
  let error = $state<string | null>(null);
  let loading = $state(false);

  // Positions affichées par la page (`numbering="index"`), à partir de 1.
  const positions = $derived(new Map(tracks.map((track, i) => [track.id, i + 1])));

  $effect(() => {
    // Autre album : tout est oublié.
    void albumId;
    open = false;
    credits = null;
    error = null;
  });

  function toggle() {
    open = !open;
    if (!open || credits || loading) return;
    const target = albumId;
    loading = true;
    error = null;
    commands
      .albumCredits(target)
      .then((c) => target === albumId && (credits = c))
      .catch((e) => {
        console.warn("crédits de l'album :", e);
        if (target === albumId) error = t.views.album.creditsError;
      })
      .finally(() => (loading = false));
  }

  /** « 1–3, 5 » ; titres absents de la page ignorés. */
  function trackList(ids: number[]): string | null {
    if (ids.length === 0) return null;
    const nums = ids
      .map((id) => positions.get(id))
      .filter((n): n is number => n !== undefined)
      .sort((a, b) => a - b);
    if (nums.length === 0) return null;
    const parts: string[] = [];
    let start = nums[0];
    let prev = start;
    for (const n of [...nums.slice(1), Infinity]) {
      if (n === prev + 1) {
        prev = n;
        continue;
      }
      parts.push(start === prev ? `${start}` : prev === start + 1 ? `${start}, ${prev}` : `${start}–${prev}`);
      start = prev = n;
    }
    const list = parts.join(", ");
    return nums.length === 1 ? t.views.album.creditsTrack(list) : t.views.album.creditsTracks(list);
  }
</script>

<div class="album-credits">
  <button class="album-credits-toggle" aria-expanded={open} onclick={toggle}>
    {t.views.album.credits}
    <span class="chevron" aria-hidden="true">{open ? "▴" : "▾"}</span>
  </button>
  {#if open}
    {#if error}
      <p class="note error">{error}</p>
    {:else if !credits}
      <p class="note">{t.views.album.creditsLoading}</p>
    {:else if credits.length === 0}
      <p class="note">{t.views.album.creditsNone}</p>
    {:else}
      <dl>
        {#each credits as group (group.role)}
          <dt>{creditRole(group.role)}</dt>
          <dd>
            {#each group.contributors as person, i (person.name)}
              {@const where = trackList(person.tracks)}
              {#if i > 0}{", "}{/if}{#if person.artistId !== null}
                {@const id = person.artistId}
                <button class="credit-artist" onclick={() => go({ name: "artist", id })}>{person.name}</button>
              {:else}{person.name}{/if}{#if where}<span class="credit-tracks">{` (${where})`}</span>{/if}
            {/each}
          </dd>
        {/each}
      </dl>
    {/if}
  {/if}
</div>

<style>
  .album-credits {
    margin: 20px 10px 0;
  }

  .album-credits-toggle {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    padding: 0;
    font-family: var(--meta-family);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--accent);
  }

  .album-credits-toggle:hover {
    text-decoration: underline;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin: 12px 0 0;
    font-size: 13px;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .credit-tracks {
    color: var(--text-muted);
    font-family: var(--meta-family);
    font-size: 12px;
  }

  .credit-artist {
    padding: 0;
    font: inherit;
    color: var(--text);
    text-align: left;
    text-decoration: underline;
    text-decoration-color: color-mix(in srgb, var(--text) 35%, transparent);
    text-underline-offset: 2px;
  }

  .credit-artist:hover {
    text-decoration-color: currentColor;
  }

  .note {
    margin: 10px 0 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .note.error {
    color: var(--danger);
  }

  :global(:root[data-platform="android"]) dl {
    grid-template-columns: 1fr;
    gap: 2px;
  }

  :global(:root[data-platform="android"]) dd {
    margin-bottom: 8px;
  }
</style>
