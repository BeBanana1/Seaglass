// Retour d'Android : ferme ce qui est au-dessus (menu, dialogue, « En lecture »), quitte
// la recherche dans une liste, revient à la page précédente, puis à l'accueil (#240) :
// seul Retour sur l'accueil quitte l'app.

import type { Shell } from "./shell.svelte";
import { closeJamDialog, jam } from "../lib/stores/jam.svelte";
import { ui } from "../lib/stores/ui.svelte";
import { closeTrackInfo, trackInfo } from "../lib/stores/trackInfo.svelte";
import { closeMenu, menu } from "../lib/stores/menu.svelte";
import { closePlaylistPicker, picker } from "../lib/stores/playlistPicker.svelte";
import { closeFolderPicker, folderPicker } from "../lib/stores/folders.svelte";
import { exitListSearch, listSearch } from "../lib/stores/listSearch.svelte";
import { selection } from "../lib/stores/selection.svelte";
import { back, canGoBack, current, replace } from "../lib/stores/nav.svelte";

export function canClose(shell: Shell): boolean {
  return (
    !!menu.open ||
    !!picker.request ||
    !!folderPicker.request ||
    !!trackInfo.track ||
    jam.dialogOpen ||
    shell.driveOpen ||
    shell.nowPlayingOpen ||
    selection.count > 0 ||
    listSearch.active ||
    canGoBack() ||
    (!!shell.session && current().name !== "home")
  );
}

export function closeTop(shell: Shell) {
  if (menu.open) closeMenu();
  else if (picker.request) closePlaylistPicker();
  else if (folderPicker.request) closeFolderPicker();
  else if (trackInfo.track) closeTrackInfo();
  else if (jam.dialogOpen) closeJamDialog();
  else if (shell.driveOpen) shell.driveOpen = false;
  // Paroles ou file en plein écran (#173) : Retour revient d'abord à « En lecture ».
  else if (shell.nowPlayingOpen && ui.nowPlayingPanel) ui.nowPlayingPanel = null;
  else if (shell.nowPlayingOpen) shell.closeNowPlaying();
  // Sélection multiple (#332) : Retour la vide avant de quitter la page.
  else if (selection.count > 0) selection.clear();
  else if (listSearch.active) exitListSearch();
  else if (canGoBack()) back();
  else replace({ name: "home" });
}
