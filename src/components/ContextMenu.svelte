<script lang="ts">
  // Menu contextuel unique (#52), ouvert par `openMenu` (lib/stores/menu.svelte.ts).
  // Se ferme sur un clic ailleurs, Échap, Tab, la molette, un redimensionnement ou un changement de page.
  import { untrack } from "svelte";
  import { closeMenu, menu, type MenuItem } from "../lib/stores/menu.svelte";
  import { nav } from "../lib/stores/nav.svelte";
  import Icon from "./Icon.svelte";

  const MARGIN = 8;

  let el = $state<HTMLElement>();
  let pos = $state({ x: 0, y: 0 });

  // Recalage dans la fenêtre une fois la taille connue, puis focus sur la première entrée.
  $effect(() => {
    const open = menu.open;
    if (!open || !el) return;
    const { width, height } = el.getBoundingClientRect();
    const x = Math.max(MARGIN, Math.min(open.x, innerWidth - width - MARGIN));
    const below = open.y + height <= innerHeight - MARGIN;
    const y = Math.max(MARGIN, below ? open.y : open.y - height);
    pos = { x, y };
    el.querySelector<HTMLElement>("[role=menuitem]")?.focus({ preventScroll: true });
  });

  $effect(() => {
    void nav.stack;
    untrack(closeMenu);
  });

  function close(restoreFocus: boolean) {
    const from = menu.open?.from;
    closeMenu();
    if (restoreFocus) from?.focus();
  }

  function run(item: MenuItem) {
    close(false);
    item.action();
  }

  function items(): HTMLElement[] {
    return Array.from(el?.querySelectorAll<HTMLElement>("[role=menuitem]") ?? []);
  }

  function onKeydown(e: KeyboardEvent) {
    const all = items();
    const i = all.indexOf(document.activeElement as HTMLElement);
    let next: number | null = null;
    if (e.key === "ArrowDown") next = (i + 1) % all.length;
    else if (e.key === "ArrowUp") next = (i - 1 + all.length) % all.length;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = all.length - 1;
    else if (e.key === "Escape" || e.key === "Tab") {
      e.preventDefault();
      e.stopPropagation();
      close(true);
      return;
    }
    if (next !== null) {
      e.preventDefault();
      e.stopPropagation();
      all[next]?.focus();
    }
  }

  // L'élément qui a ouvert le menu gère lui-même un second clic (bouton « … » = bascule).
  function outside(target: EventTarget | null): boolean {
    const node = target as Node | null;
    return !el?.contains(node) && !menu.open?.from?.contains(node);
  }

  function onPointerdown(e: PointerEvent) {
    if (menu.open && outside(e.target)) closeMenu();
  }

  function onWheel(e: WheelEvent) {
    if (menu.open && !el?.contains(e.target as Node)) closeMenu();
  }

  function dismiss() {
    if (menu.open) closeMenu();
  }
</script>

<svelte:window onpointerdowncapture={onPointerdown} onwheel={onWheel} onresize={dismiss} onblur={dismiss} />

{#if menu.open}
  <div
    class="context-menu"
    role="menu"
    tabindex="-1"
    bind:this={el}
    style:left="{pos.x}px"
    style:top="{pos.y}px"
    onkeydown={onKeydown}
    oncontextmenu={(e) => e.preventDefault()}
  >
    <!-- Sans clé : un menu rouvert par-dessus un autre doit remplacer ses entrées. -->
    {#each menu.open.entries as entry}
      {#if entry === "separator"}
        <div class="context-menu-sep" role="separator"></div>
      {:else}
        <button class="context-menu-item" role="menuitem" tabindex="-1" onclick={() => run(entry)}>
          {#if entry.icon}<Icon name={entry.icon} size={16} />{:else}<span class="no-icon"></span>{/if}
          <span>{entry.label}</span>
        </button>
      {/if}
    {/each}
  </div>
{/if}

<style>
  .context-menu {
    position: fixed;
    z-index: 100;
    display: flex;
    flex-direction: column;
    min-width: 210px;
    max-width: 320px;
    padding: 4px;
    border: var(--dialog-border);
    border-radius: var(--popover-radius);
    background: var(--dialog-surface);
    backdrop-filter: var(--dialog-blur);
    box-shadow: var(--popover-shadow);
    font-size: 13px;
  }

  .context-menu:focus {
    outline: none;
  }

  .context-menu-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 10px;
    border-radius: max(0px, calc(var(--radius) - 4px));
    color: var(--text);
    text-align: left;
  }

  .context-menu-item span:last-child {
    overflow: hidden;
    white-space: nowrap;
    text-overflow: ellipsis;
  }

  .context-menu-item :global(svg) {
    color: var(--text-muted);
  }

  .context-menu-item:hover,
  .context-menu-item:focus-visible,
  .context-menu-item:focus {
    outline: none;
    background: var(--bg-hover);
  }

  .context-menu-item:hover :global(svg),
  .context-menu-item:focus :global(svg) {
    color: var(--accent);
  }

  .no-icon {
    width: 16px;
  }

  .context-menu-sep {
    height: 1px;
    margin: 4px 6px;
    background: var(--border);
  }
</style>
