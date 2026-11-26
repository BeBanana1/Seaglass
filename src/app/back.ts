// Android Back: closes what is on top (menu, dialog, "Now playing"), leaves the search in a
// list, goes back to the previous page, then to the home page (#240): only Back on the home
// page leaves the app.

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
  // Lyrics or queue in full screen (#173): Back first returns to "Now playing".
  else if (shell.nowPlayingOpen && ui.nowPlayingPanel) ui.nowPlayingPanel = null;
  else if (shell.nowPlayingOpen) shell.closeNowPlaying();
  // Multiple selection (#332): Back clears it before leaving the page.
  else if (selection.count > 0) selection.clear();
  else if (listSearch.active) exitListSearch();
  else if (canGoBack()) back();
  else replace({ name: "home" });
}
