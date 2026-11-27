// Pasted TIDAL links (#203): the backend recognizes the link and gives the page to open; a
// track opens its album, track highlighted. On Android, the system's "Share" menu
// goes through the backend, which keeps the page until the UI takes it (`openSharedLink`).

import { commands, type LinkTarget } from "./ipc";
import { go } from "./stores/nav.svelte";
import { pushToast } from "./stores/toasts.svelte";

function openTarget(target: LinkTarget) {
  switch (target.kind) {
    case "album":
      go({ name: "album", id: target.id, track: target.track ?? undefined });
      break;
    case "artist":
      go({ name: "artist", id: target.id });
      break;
    case "playlist":
      go({ name: "playlist", uuid: target.uuid });
      break;
    case "mix":
      go({ name: "mix", id: target.id });
      break;
    case "import":
      go({ name: "import", link: target.link });
      break;
    case "feed":
      go({ name: "feed" });
      break;
  }
}

/** Opens the page of the link contained in `text`; `false` if there is none. */
export async function openTidalLink(text: string): Promise<boolean> {
  try {
    const target = await commands.openLink(text);
    if (!target) return false;
    openTarget(target);
    return true;
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
    return true;
  }
}

/** Link shared to the app (Android, #203) or page of a tapped notification (#435), to
 * the `open-link` event or at startup. */
export async function openSharedLink() {
  const target = await commands.takeSharedLink().catch(() => null);
  if (target) openTarget(target);
}
