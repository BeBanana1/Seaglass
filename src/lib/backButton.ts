// Touche (ou geste) Retour d'Android (#24). `MainActivity` appelle `window.__seaglassBack()` à
// chaque Retour : vrai si l'app a fermé quelque chose (menu, vue, page), faux sinon, et
// Android passe alors l'app en arrière-plan (la lecture continue).
//
// Plus d'entrée d'historique sentinelle (jusqu'en 0.4.117) : la WebView ignore pour Retour les
// entrées posées sans geste de l'utilisateur dans la page, donc le mode conduite ouvert par le
// lien `seaglass://drive` faisait quitter l'app au premier Retour (#193, vu sur le S21).

declare global {
  interface Window {
    __seaglassBack?: () => boolean;
  }
}

export function initBackButton(canClose: () => boolean, close: () => void) {
  window.__seaglassBack = () => {
    if (!canClose()) return false;
    close();
    return true;
  };
}
