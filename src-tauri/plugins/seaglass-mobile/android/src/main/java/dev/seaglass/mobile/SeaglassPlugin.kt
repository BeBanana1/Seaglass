package dev.seaglass.mobile

import android.app.Activity
import android.app.Dialog
import android.graphics.Bitmap
import android.view.WindowManager
import android.webkit.CookieManager
import android.webkit.WebResourceRequest
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import app.tauri.annotation.Command
import app.tauri.annotation.InvokeArg
import app.tauri.annotation.TauriPlugin
import app.tauri.plugin.Channel
import app.tauri.plugin.Invoke
import app.tauri.plugin.JSObject
import app.tauri.plugin.Plugin

@InvokeArg
class LoginArgs {
    lateinit var url: String
    lateinit var redirectPrefix: String
}

@InvokeArg
class ChannelArgs {
    lateinit var channel: Channel
}

/** Une opération de `engine::Op` (champs selon `op`), lue par `Bridge`. */
class EngineArgs {
    lateinit var op: String
    var url: String? = null
    var start: Double? = null
    var gain: Double? = null
    var paused: Boolean? = null
    var position: Double? = null
    var on: Boolean? = null
    var level: Int? = null
    var muted: Boolean? = null
    /** Égaliseur (#467) avec `play` et `currentGain` ; `null` = désactivé. */
    internal var eq: EqSpec? = null
}

@InvokeArg
class ScreenArgs {
    var on: Boolean = false
}

@InvokeArg
class InstallArgs {
    lateinit var path: String
}

/**
 * Commandes qui ont besoin de l'activité, ou qui ne tournent qu'au lancement (canaux des
 * événements). Celles que le lecteur, la session média et les secrets envoient en cours de
 * route passent par `Bridge` (#169) : l'app peut alors avoir été balayée des récents.
 */
@TauriPlugin
class SeaglassPlugin(private val activity: Activity) : Plugin(activity) {
    /** WebView de l'app : la hauteur du clavier lui est transmise (#251). */
    override fun load(webView: WebView) {
        KeyboardInset.watch(webView)
    }

    /** Installe l'APK de mise à jour déjà vérifié par Rust (`apk_update.rs`). */
    @Command
    fun installApk(invoke: Invoke) {
        val args = invoke.parseArgs(InstallArgs::class.java)
        Playback.onMain {
            ApkInstaller.install(activity, args.path) { error ->
                if (error == null) invoke.resolve() else invoke.reject(error)
            }
        }
    }

    /** Mode conduite (#193) : l'écran reste allumé tant qu'il est ouvert. */
    @Command
    fun keepScreenOn(invoke: Invoke) {
        val args = invoke.parseArgs(ScreenArgs::class.java)
        activity.runOnUiThread {
            val flag = WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
            if (args.on) activity.window.addFlags(flag) else activity.window.clearFlags(flag)
            invoke.resolve()
        }
    }

    /** Crée le lecteur (une fois par processus) et renvoie ses événements sur `channel`. */
    @Command
    fun engineStart(invoke: Invoke) {
        val args = invoke.parseArgs(ChannelArgs::class.java)
        val context = activity.applicationContext
        Playback.onMain {
            Playback.start(context, args.channel)
            invoke.resolve()
        }
    }

    /** Actions de la session média (notification, écouteurs), renvoyées sur `channel`. */
    @Command
    fun sessionStart(invoke: Invoke) {
        val args = invoke.parseArgs(ChannelArgs::class.java)
        Playback.onMain {
            Playback.attachSession(args.channel)
            invoke.resolve()
        }
    }

    /**
     * Connexion TIDAL (#24) : Android n'a qu'une WebView pour l'app, la page de connexion
     * s'ouvre donc dans une WebView à part, sans IPC Tauri. La première navigation vers la
     * redirection est bloquée et son URL rendue à Rust, qui en tire le code PKCE.
     */
    @Command
    fun login(invoke: Invoke) {
        val args = invoke.parseArgs(LoginArgs::class.java)
        activity.runOnUiThread {
            clearLoginData()
            val webView = WebView(activity)
            val dialog = Dialog(activity, android.R.style.Theme_DeviceDefault_NoActionBar)
            var done = false

            fun finish(url: String?) {
                if (done) return
                done = true
                webView.stopLoading()
                dialog.dismiss()
                webView.destroy()
                clearLoginData()
                val result = JSObject()
                if (url != null) result.put("url", url)
                invoke.resolve(result)
            }

            webView.settings.javaScriptEnabled = true
            webView.settings.domStorageEnabled = true
            // Pages web seulement (#145) : ni fichiers de l'appareil (vrai par défaut avant
            // Android 11) ni fournisseurs de contenu.
            webView.settings.allowFileAccess = false
            webView.settings.allowContentAccess = false
            webView.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val url = request.url.toString()
                    if (url.startsWith(args.redirectPrefix)) {
                        finish(url)
                        return true
                    }
                    // Tout autre schéma (`intent:`, `file:`, `javascript:`…) est bloqué.
                    return request.url.scheme != "https"
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

    /**
     * Données de la page de connexion (#258), avant et après usage : la session web de TIDAL
     * ne survit pas, et une nouvelle connexion peut prendre un autre compte. `CookieManager`
     * est commun à tout le processus, mais l'interface de l'app n'a pas de cookies ; le
     * stockage web n'est effacé que pour les origines de TIDAL.
     */
    private fun clearLoginData() {
        CookieManager.getInstance().removeAllCookies(null)
        CookieManager.getInstance().flush()
        val storage = WebStorage.getInstance()
        for (origin in TIDAL_ORIGINS) storage.deleteOrigin(origin)
    }

    private companion object {
        val TIDAL_ORIGINS = listOf("https://login.tidal.com", "https://tidal.com", "https://listen.tidal.com")
    }
}
