package dev.seaglass.app

import android.os.Bundle
import android.webkit.WebView
import androidx.activity.OnBackPressedCallback
import androidx.activity.enableEdgeToEdge

class MainActivity : TauriActivity() {
  // Back is asked of the UI (`src/lib/backButton.ts`): `window.__seaglassBack()` closes what
  // is open and returns true, otherwise Android receives Back and sends the app to the
  // background. Not through the WebView history (Wry's `handleBackNavigation`): it ignores
  // entries pushed without a user gesture, such as those of a view opened by a link (#193).
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
