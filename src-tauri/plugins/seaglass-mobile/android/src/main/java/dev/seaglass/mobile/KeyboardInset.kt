package dev.seaglass.mobile

import android.webkit.WebView
import androidx.core.view.ViewCompat
import androidx.core.view.WindowInsetsCompat

/**
 * Keyboard height for the UI (#251). The app is edge to edge (`enableEdgeToEdge`, enforced
 * since Android 15): the keyboard covers the WebView without resizing it, and the page
 * does not see it (`visualViewport` unchanged). So we set the keyboard height, in CSS
 * pixels, in the `--keyboard-inset` variable of `<html>` (0 when it is closed).
 *
 * Purely additive listening: `OnGlobalLayoutListener` rereads the window insets on every
 * layout pass (opening the keyboard triggers one) without replacing the insets listener
 * of the WebView or of its parent.
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
