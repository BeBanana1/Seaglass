// Menu contextuel unique de l'app (clic droit, bouton « … »), affiché par components/ContextMenu.svelte.

import type { IconName } from "../../components/Icon.svelte";

export interface MenuItem {
  label: string;
  icon?: IconName;
  action: () => unknown;
}

export type MenuEntry = MenuItem | "separator";

export interface OpenMenu {
  /** Coordonnées de la fenêtre (pixels CSS) ; le menu se recale s'il déborde. */
  x: number;
  y: number;
  entries: MenuEntry[];
  /** Élément qui a ouvert le menu : il reprend le focus à la fermeture au clavier. */
  from: HTMLElement | null;
  /** Élément concerné (ligne de titre), mis en évidence tant que le menu est ouvert. */
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
 * Position d'un menu ouvert par un événement `contextmenu` : au curseur, ou sous l'élément
 * visé quand il vient du clavier (touche Menu, Maj+F10), qui ne donne pas de coordonnées.
 */
export function menuPosition(e: MouseEvent, fallback: HTMLElement): { x: number; y: number } {
  if (e.clientX !== 0 || e.clientY !== 0) return { x: e.clientX, y: e.clientY };
  const rect = fallback.getBoundingClientRect();
  return { x: rect.left, y: rect.bottom };
}
