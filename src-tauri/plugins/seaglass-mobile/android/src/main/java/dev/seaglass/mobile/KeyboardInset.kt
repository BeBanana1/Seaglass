package dev.seaglass.mobile

import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Hauteur du clavier pour l'UI (#251). L'app est bord à bord (`enableEdgeToEdge`, imposé
 * depuis Android 15) : le clavier recouvre la WebView sans la redimensionner, et la page
 * ne le voit pas (`visualViewport` inchangé). On pose donc la hauteur du clavier, en pixels
 * CSS, dans la variable `--keyboard-inset` de `<html>` (0 quand il est fermé).
 *
 * Écoute purement additive : `OnGlobalLayoutListener` relit les encarts de la fenêtre à chaque
 * mise en page (l'ouverture du clavier en provoque une) sans remplacer l'écouteur d'encarts
 * de la WebView ni de son parent.
 */
object KeyboardInset {
    fun watch(webView: WebView) {
        var last = -1
        webView.viewTreeObserver.addOnGlobalLayoutListener {
            val insets = ViewCompat.getRootWindowInsets(webView) ?: return@addOnGlobalLayoutListener
            val px = insets.getInsets(WindowInsetsCompat.Type.ime()).bottom
            val css = Math.round(px / webView.resources.displayMetrics.density)
            if (css == last) return@addOnGlobalLayoutListener
            last = css
            webView.evaluateJavascript(
                "document.documentElement.style.setProperty('--keyboard-inset','${css}px')",
                null,
            )
        }
    }
}
