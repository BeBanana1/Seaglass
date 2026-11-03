package dev.seaglass.mobile

import android.app.Activity
import android.app.Dialog
import android.graphics.Bitmap
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class LoginArgs {
    lateinit var url: String
    lateinit var redirectPrefix: String
}

@TauriPlugin
class SeaglassPlugin(private val activity: Activity) : Plugin(activity) {
    /**
     * Connexion TIDAL (#24) : Android n'a qu'une WebView pour l'app, la page de connexion
     * s'ouvre donc dans une WebView à part, sans IPC Tauri. La première navigation vers la
     * redirection est bloquée et son URL rendue à Rust, qui en tire le code PKCE.
     */
    @Command
    fun login(invoke: Invoke) {
        val args = invoke.parseArgs(LoginArgs::class.java)
        activity.runOnUiThread {
            val webView = WebView(activity)
            val dialog = Dialog(activity, android.R.style.Theme_DeviceDefault_NoActionBar)
            var done = false

            fun finish(url: String?) {
                if (done) return
                done = true
                webView.stopLoading()
                dialog.dismiss()
                webView.destroy()
                val result = JSObject()
                if (url != null) result.put("url", url)
                invoke.resolve(result)
            }

            webView.settings.javaScriptEnabled = true
            webView.settings.domStorageEnabled = true
            webView.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val url = request.url.toString()
                    if (url.startsWith(args.redirectPrefix)) {
                        finish(url)
                        return true
                    }
                    return false
                }

                // Filet : une redirection qui ne passe pas par shouldOverrideUrlLoading.
                override fun onPageStarted(view: WebView, url: String, favicon: Bitmap?) {
                    if (url.startsWith(args.redirectPrefix)) finish(url)
                }
            }
            dialog.setContentView(webView)
            dialog.setOnCancelListener { finish(null) }
            dialog.show()
            webView.loadUrl(args.url)
        }
    }
}
