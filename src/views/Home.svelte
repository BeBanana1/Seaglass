<script lang="ts">
  import { onMount } from "svelte";
  import { commands, type AlbumSummary, type HomeRow } from "../lib/ipc";
  import CardGrid from "../components/CardGrid.svelte";
  import Hero from "../components/Hero.svelte";

  // Gardé entre deux visites de l'accueil pour éviter de recharger à chaque retour.
  let rows = $state<HomeRow[] | null>(cached);
  let error = $state<string | null>(null);

  /** Premier album de l'accueil, mis en avant avec le titre de sa ligne. */
  const featured = $derived.by((): { album: AlbumSummary; context: string } | null => {
    for (const row of rows ?? []) {
      const item = row.items.find((i) => i.kind === "album");
      if (item?.kind === "album") return { album: item, context: row.title };
    }
    return null;
  });

  onMount(async () => {
    if (rows) return;
    try {
      rows = cached = await commands.home();
    } catch (e) {
      error = String(e);
    }
  });
</script>

<script lang="ts" module>
  let cached: HomeRow[] | null = null;
</script>

<section>
  <h1>Accueil</h1>
  {#if error}
    <p class="error">{error}</p>
  {:else if !rows}
    <p class="muted">Chargement…</p>
  {:else}
    {#if featured}<Hero album={featured.album} context={featured.context} />{/if}
    {#each rows as row, i (i)}
      <CardGrid title={row.title} items={row.items} row />
    {/each}
  {/if}
</section>

<style>
  section {
    padding: 24px;
  }

  h1 {
    margin: 0 10px 20px;
    font-size: 26px;
  }

  .muted {
    margin: 0 10px;
    color: var(--text-muted);
  }

  .error {
    margin: 0 10px;
    color: var(--danger);
  }
</style>
