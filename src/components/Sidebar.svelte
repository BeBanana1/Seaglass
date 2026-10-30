<script lang="ts">
  import { current, replace, type Route } from "../lib/stores/nav.svelte";

  const entries: { label: string; route: Route; match: Route["name"][] }[] = [
    { label: "Accueil", route: { name: "home" }, match: ["home"] },
    { label: "Recherche", route: { name: "search" }, match: ["search"] },
    { label: "Bibliothèque", route: { name: "library", tab: "playlists" }, match: ["library"] },
    { label: "Réglages", route: { name: "settings", section: "appearance" }, match: ["settings"] },
  ];

  const active = $derived(current().name);
</script>

<nav>
  {#each entries as entry (entry.label)}
    <button
      class:active={entry.match.includes(active)}
      class:settings={entry.route.name === "settings"}
      onclick={() => replace(entry.route)}
    >
      {entry.label}
    </button>
  {/each}
</nav>

<style>
  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    box-sizing: border-box;
    height: 100%;
    padding: 16px 10px;
    border-right: 1px solid var(--border);
  }

  button {
    padding: 8px 12px;
    border-radius: var(--radius);
    text-align: left;
    font-weight: 500;
    color: var(--text-muted);
  }

  button:hover {
    color: var(--text);
    background: var(--bg-hover);
  }

  button.active {
    color: var(--text);
    background: var(--bg-raised);
  }

  /* Réglages en bas de la barre, à l'écart de la navigation dans le catalogue. */
  button.settings {
    margin-top: auto;
  }
</style>
