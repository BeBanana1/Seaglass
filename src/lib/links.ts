// Liens TIDAL collés (#203) : le backend reconnaît le lien et donne la page à ouvrir ; un
// titre ouvre son album, titre mis en évidence.

import { commands } from "./ipc";
import { go } from "./stores/nav.svelte";
import { pushToast } from "./stores/toasts.svelte";

/** Ouvre la page du lien contenu dans `text` ; `false` s'il n'y en a pas. */
export async function openTidalLink(text: string): Promise<boolean> {
  try {
    const target = await commands.openLink(text);
    if (!target) return false;
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
    }
    return true;
  } catch (e) {
    pushToast({ level: "error", message: String(e) });
    return true;
  }
}
