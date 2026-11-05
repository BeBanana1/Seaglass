// Liens TIDAL collés (#203) : le backend reconnaît le lien et donne la page à ouvrir ; un
// titre ouvre son album, titre mis en évidence. Sur Android, le menu « Partager » du système
// passe par le backend, qui garde la page jusqu'à ce que l'UI la prenne (`openSharedLink`).

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
  }
}

/** Ouvre la page du lien contenu dans `text` ; `false` s'il n'y en a pas. */
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

/** Lien partagé vers l'app (Android, #203), à l'événement `open-link` ou au démarrage. */
export async function openSharedLink() {
  const target = await commands.takeSharedLink().catch(() => null);
  if (target) openTarget(target);
}
