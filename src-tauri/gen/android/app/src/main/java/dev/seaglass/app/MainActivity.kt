package dev.seaglass.app

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  // Retour est demandé à l'UI (`src/lib/backButton.ts`) : `window.__seaglassBack()` ferme ce
  // qui est ouvert et renvoie vrai, sinon Android reçoit Retour et passe l'app en arrière-plan.
  // Pas via l'historique de la WebView (`handleBackNavigation` de Wry) : elle ignore les
  // entrées posées sans geste de l'utilisateur, celles d'une vue ouverte par un lien (#193).
  override val handleBackNavigation: Boolean = false

  override fun onCreate(savedInstanceState: Bundle?) {
    enableEdgeToEdge()
    super.onCreate(savedInstanceState)
  }

  override fun onWebViewCreate(webView: WebView) {
    val callback = object : OnBackPressedCallback(true) {
      override fun handleOnBackPressed() {
        webView.evaluateJavascript("window.__seaglassBack ? window.__seaglassBack() : false") { result ->
          if (result != "true") {
            isEnabled = false
            onBackPressedDispatcher.onBackPressed()
            isEnabled = true
          }
        }
      }
    }
    onBackPressedDispatcher.addCallback(this, callback)
  }
}
