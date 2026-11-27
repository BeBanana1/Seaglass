// The app's single context menu (right click, "…" button), shown by
// components/ContextMenu.svelte.

import type { IconName } from "../../components/Icon.svelte";

export interface MenuItem {
  label: string;
  icon?: IconName;
  /** Choice in a list of options (sort, #182): a check mark instead of the icon if true. */
  checked?: boolean;
  action: () => unknown;
}

export type MenuEntry = MenuItem | "separator";

export interface OpenMenu {
  /** Window coordinates (CSS pixels); the menu moves back in if it overflows. */
  x: number;
  y: number;
  entries: MenuEntry[];
  /** Element that opened the menu: it gets the focus back on a keyboard close. */
  from: HTMLElement | null;
  /** Element the menu is about (track row), highlighted while the menu is open. */
  owner?: HTMLElement;
}

export const menu = $state<{ open: OpenMenu | null }>({ open: null });

export function openMenu(open: OpenMenu) {
  menu.open = open;
}

export function closeMenu() {
  menu.open = null;
}

/**
 * Position of a menu opened by a `contextmenu` event: at the cursor, or under the target
 * element when it comes from the keyboard (Menu key, Shift+F10), which gives no coordinates.
 */
export function menuPosition(e: MouseEvent, fallback: HTMLElement): { x: number; y: number } {
  if (e.clientX !== 0 || e.clientY !== 0) return { x: e.clientX, y: e.clientY };
  const rect = fallback.getBoundingClientRect();
  return { x: rect.left, y: rect.bottom };
}
