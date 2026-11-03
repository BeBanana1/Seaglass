<script lang="ts">
  // Navigation du bas sur Android (#24) : remplace la barre latérale du bureau.
  import { current, replace, type Route } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  type TabKey = keyof typeof t.common.tabs;

  const tabs: { key: TabKey; icon: IconName; route: Route }[] = [
    { key: "home", icon: "home", route: { name: "home" } },
    { key: "feed", icon: "feed", route: { name: "feed" } },
    { key: "search", icon: "search", route: { name: "search" } },
    { key: "library", icon: "library", route: { name: "library", tab: "playlists" } },
    { key: "settings", icon: "settings", route: { name: "settings", section: "appearance" } },
  ];

  // Les pages ouvertes depuis un onglet (album, artiste…) n'en allument aucun.
  const active = $derived(current().name);
</script>

<nav class="bottom-nav">
  {#each tabs as tab (tab.key)}
    <button
      class="tab"
      class:active={active === tab.route.name}
      aria-current={active === tab.route.name ? "page" : undefined}
      onclick={() => replace(tab.route)}
    >
      <Icon name={tab.icon} size={22} />
      <span class="tab-label">{t.common.tabs[tab.key]}</span>
    </button>
  {/each}
</nav>

<style>
  .bottom-nav {
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    padding: 4px 4px var(--safe-bottom);
  }

  .tab {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 3px;
    min-width: 0;
    padding: 8px 2px 6px;
    color: var(--text-muted);
  }

  .tab.active {
    color: var(--accent);
  }

  .tab-label {
    max-width: 100%;
    overflow: hidden;
    font-size: 11px;
    font-weight: 500;
    white-space: nowrap;
    text-overflow: ellipsis;
  }
</style>
