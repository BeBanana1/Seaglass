<script lang="ts">
  import type { Pin } from "../lib/ipc";
  import { current, replace, type Route } from "../lib/stores/nav.svelte";
  import { togglePin, ui } from "../lib/stores/ui.svelte";
  import { pushToast } from "../lib/stores/toasts.svelte";
  import Icon from "./Icon.svelte";

  const entries: { label: string; route: Route; match: Route["name"][] }[] = [
    { label: "Accueil", route: { name: "home" }, match: ["home"] },
    { label: "Fil d'actualité", route: { name: "feed" }, match: ["feed"] },
    { label: "Recherche", route: { name: "search" }, match: ["search"] },
    { label: "Bibliothèque", route: { name: "library", tab: "playlists" }, match: ["library"] },
    { label: "Réglages", route: { name: "settings", section: "appearance" }, match: ["settings"] },
  ];

  const route = $derived(current());

  const pinRoute = (pin: Pin): Route =>
    pin.kind === "playlist" ? { name: "playlist", uuid: pin.uuid } : { name: "library", tab: "tracks" };

  const pinLabel = (pin: Pin) => (pin.kind === "playlist" ? pin.title || "Playlist" : "Titres likés");

  function pinActive(pin: Pin): boolean {
    const r = route;
    return pin.kind === "playlist"
      ? r.name === "playlist" && r.uuid === pin.uuid
      : r.name === "library" && r.tab === "tracks";
  }

  async function unpin(pin: Pin) {
    try {
      await togglePin(pin);
    } catch (e) {
      pushToast({ level: "error", message: String(e) });
    }
  }
</script>

<nav>
  {#each entries as entry (entry.label)}
    <button
      class:active={entry.match.includes(route.name)}
      class:settings={entry.route.name === "settings"}
      onclick={() => replace(entry.route)}
    >
      {entry.label}
    </button>
    {#if entry.route.name === "library" && ui.settings.pins.length > 0}
      <div class="pins">
        <span class="pins-title">Épinglés</span>
        {#each ui.settings.pins as pin (pin.kind === "playlist" ? pin.uuid : pin.kind)}
          <div class="pin" class:active={pinActive(pin)}>
            <button class="pin-open" onclick={() => replace(pinRoute(pin))} title={pinLabel(pin)}>
              {pinLabel(pin)}
            </button>
            <button
              class="pin-remove"
              onclick={() => unpin(pin)}
              title="Désépingler"
              aria-label={`Désépingler ${pinLabel(pin)}`}
            >
              <Icon name="close" size={14} />
            </button>
          </div>
        {/each}
      </div>
    {/if}
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

  /* Les épingles défilent seules : la barre garde Réglages en bas. */
  .pins {
    display: flex;
    flex-direction: column;
    gap: 2px;
    min-height: 0;
    margin-top: 14px;
    overflow-y: auto;
  }

  .pins-title {
    padding: 0 12px 4px;
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
  }

  .pin {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
  }

  .pin:hover {
    background: var(--bg-hover);
  }

  .pin.active {
    background: var(--bg-raised);
  }

  .pin-open {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
  }

  .pin-open:hover,
  .pin-remove:hover {
    background: none;
  }

  .pin.active .pin-open {
    color: var(--text);
  }

  .pin-remove {
    display: flex;
    padding: 6px;
    visibility: hidden;
  }

  .pin:hover .pin-remove,
  .pin-remove:focus-visible {
    visibility: visible;
  }
</style>
