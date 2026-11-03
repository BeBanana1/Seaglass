// Touche (ou geste) Retour d'Android (#24). L'activité ne la passe à la WebView que si son
// historique peut reculer : une entrée sentinelle est posée tant que l'app a quelque chose à
// fermer ou une page précédente. Retour la consomme (`popstate`) et l'app fait l'action. Sans
// sentinelle, Android reçoit Retour et passe l'app en arrière-plan (la lecture continue).

let armed = false;
/** Sentinelle en cours de retrait par `history.back()` : son `popstate` n'est pas un Retour. */
let dropping = false;
let canGoBack: () => boolean = () => false;

function sync() {
  if (dropping) return;
  if (canGoBack() && !armed) {
    history.pushState({ seaglass: true }, "");
    armed = true;
  } else if (!canGoBack() && armed) {
    armed = false;
    dropping = true;
    history.back();
  }
}

export function initBackButton(can: () => boolean, goBack: () => void) {
  canGoBack = can;
  window.addEventListener("popstate", () => {
    if (dropping) {
      dropping = false;
    } else {
      armed = false;
      goBack();
    }
    sync();
  });
}

/** À appeler quand ce que Retour peut fermer change. */
export const syncBackButton = sync;
