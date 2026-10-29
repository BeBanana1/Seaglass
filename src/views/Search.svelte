<script lang="ts">
  import { commands, type Track } from "../lib/ipc";
  import { player } from "../lib/stores/player.svelte";
  import TrackRow from "../components/TrackRow.svelte";

  let query = $state("");
  let results = $state<Track[]>([]);
  let searching = $state(false);
  let error = $state<string | null>(null);
  let searched = $state(false);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    searching = true;
    error = null;
    try {
      results = await commands.search(query);
      searched = true;
    } catch (err) {
      error = String(err);
    } finally {
      searching = false;
    }
  }

  /** Les résultats forment le contexte de lecture. */
  async function play(index: number) {
    try {
      await commands.playTracks(
        results.map((t) => t.id),
        index,
      );
    } catch (err) {
      error = String(err);
    }
  }
</script>

<section>
  <form onsubmit={submit}>
    <input
      type="search"
      placeholder="Titres, artistes…"
      bind:value={query}
      autocomplete="off"
      spellcheck="false"
    />
  </form>

  {#if error}
    <p class="error">{error}</p>
  {:else if searching}
    <p class="muted">Recherche…</p>
  {:else if searched && results.length === 0}
    <p class="muted">Aucun résultat.</p>
  {/if}

  <div role="list">
    {#each results as track, i (track.id)}
      <TrackRow {track} active={player.state.track?.id === track.id} onPlay={() => play(i)} />
    {/each}
  </div>
</section>

<style>
  section {
    padding: 20px;
  }

  form {
    margin-bottom: 16px;
  }

  input {
    width: 100%;
    max-width: 520px;
    padding: 10px 14px;
    border: 1px solid var(--border);
    border-radius: 999px;
    background: var(--bg-raised);
    user-select: text;
  }

  input::placeholder {
    color: var(--text-muted);
  }

  .muted {
    color: var(--text-muted);
  }

  .error {
    color: var(--danger);
  }
</style>
