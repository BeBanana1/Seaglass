<script lang="ts">
  // Bottom navigation on Android (#24): replaces the desktop sidebar.
  // Liked tracks have their own tab (#143), the settings are in the header.
  import { current, replace, type Route } from "../lib/stores/nav.svelte";
  import { t } from "../lib/i18n.svelte";
  import { feed } from "../lib/stores/feed.svelte";
  import Icon, { type IconName } from "./Icon.svelte";

  type TabKey = keyof typeof t.common.tabs;

  const tabs: { key: TabKey; icon: IconName; route: Route }[] = [
    { key: "home", icon: "home", route: { name: "home" } },
    { key: "likes", icon: "heart", route: { name: "library", tab: "tracks" } },
    { key: "search", icon: "search", route: { name: "search" } },
    { key: "library", icon: "library", route: { name: "library", tab: "playlists" } },
    { key: "feed", icon: "feed", route: { name: "feed" } },
  ];

  // Pages opened from a tab (album, artist…) light none of them.
  // Library › Tracks is the Likes tab, the other library tabs are the Library tab.
  function isActive(route: Route): boolean {
    const here = current();
    // Statistics (#326), smart playlists (#383) and folders (#331): library pages.
    if (
      here.name === "stats" ||
      here.name === "recap" ||
      here.name === "smart" ||
      here.name === "folder"
    )
      return route.name === "library" && route.tab !== "tracks";
    if (here.name !== route.name) return false;
    if (here.name === "library" && route.name === "library")
      return (here.tab === "tracks") === (route.tab === "tracks");
    return true;
  }
</script>

<nav class="bottom-nav">
  {#each tabs as tab (tab.key)}
    {@const active = isActive(tab.route)}
    <button
      class="tab"
      class:active
      aria-current={active ? "page" : undefined}
      onclick={() => replace(tab.route)}
    >
      <span class="tab-icon">
        <Icon name={tab.icon} size={22} />
        {#if tab.key === "feed" && feed.unseen > 0}
          <span class="tab-dot" aria-label={t.views.feed.unseen(feed.unseen)}></span>
        {/if}
      </span>
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

  .tab-icon {
    position: relative;
    display: flex;
  }

  /* Feed news not seen yet (#415). */
  .tab-dot {
    position: absolute;
    top: -1px;
    right: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--accent);
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
