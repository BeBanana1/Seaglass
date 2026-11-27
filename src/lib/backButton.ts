// Android's Back key (or gesture) (#24). `MainActivity` calls `window.__seaglassBack()` on
// every Back: true if the app closed something (menu, view, page), false otherwise, and
// Android then sends the app to the background (playback continues).
//
// No more sentinel history entry (until 0.4.117): for Back, the WebView ignores entries
// pushed without a user gesture in the page, so the driving mode opened by the
// `seaglass://drive` link made the app quit on the first Back (#193, seen on the S21).

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
