// #330: library export (desktop). The backend reads the lists and writes the files; here,
// only the button's state, the progress and the messages.
import { commands, events } from "../ipc";
import { t } from "../i18n.svelte";
import { pushToast } from "./toasts.svelte";

export const libraryExport = $state<{ running: boolean; done: number; total: number }>({
  running: false,
  done: 0,
  total: 0,
});

export async function startLibraryExport() {
  if (libraryExport.running) return;
  libraryExport.running = true;
  libraryExport.done = 0;
  libraryExport.total = 0;
  const unlisten = await events.onLibraryExport((p) => {
    libraryExport.done = p.done;
    libraryExport.total = p.total;
  });
  try {
    const report = await commands.exportLibrary();
    pushToast({ level: "info", message: t.settings.exportDone(report.playlists, report.tracks) });
    if (report.failed.length) {
      pushToast({ level: "error", message: t.settings.exportFailed(report.failed) });
    }
    if (report.truncated) pushToast({ level: "info", message: t.settings.exportTruncated });
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
  } finally {
    unlisten();
    libraryExport.running = false;
  }
}

export function cancelLibraryExport() {
  void commands.cancelLibraryExport();
}
