package dev.seaglass.mobile

import android.app.Activity
import android.app.Dialog
import android.graphics.Bitmap
import android.net.Uri
import android.webkit.WebResourceRequest
import android.webkit.WebView
import android.webkit.WebViewClient
import androidx.media3.common.MediaMetadata
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

/** Une opération de `engine::Op` (champs selon `op`). */
@InvokeArg
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
}

@InvokeArg
class NowPlayingArgs {
    var title: String? = null
    var artist: String? = null
    var album: String? = null
    var artworkUrl: String? = null
}

@InvokeArg
class SecretArgs {
    lateinit var name: String
    var value: String? = null
}

@InvokeArg
class InstallArgs {
    lateinit var path: String
}

@TauriPlugin
class SeaglassPlugin(private val activity: Activity) : Plugin(activity) {
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

    private val secrets by lazy { SecretStore(activity.applicationContext) }

    /** Secret chiffré par le Keystore ; `value` absente s'il n'existe pas. */
    @Command
    fun secretGet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        try {
            val result = JSObject()
            secrets.get(args.name)?.let { result.put("value", it) }
            invoke.resolve(result)
        } catch (e: Exception) {
            invoke.reject("lecture du secret impossible : ${e.javaClass.simpleName}")
        }
    }

    @Command
    fun secretSet(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        try {
            secrets.set(args.name, args.value ?: throw IllegalArgumentException("valeur manquante"))
            invoke.resolve()
        } catch (e: Exception) {
            invoke.reject("écriture du secret impossible : ${e.javaClass.simpleName}")
        }
    }

    /** `existed` : faux si le secret n'existait pas. */
    @Command
    fun secretDelete(invoke: Invoke) {
        val args = invoke.parseArgs(SecretArgs::class.java)
        val result = JSObject()
        result.put("existed", secrets.delete(args.name))
        invoke.resolve(result)
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

    @Command
    fun engine(invoke: Invoke) {
        val args = invoke.parseArgs(EngineArgs::class.java)
        Playback.onMain {
            try {
                Playback.apply(args)
                invoke.resolve()
            } catch (e: Exception) {
                invoke.reject(e.message ?: e.toString())
            }
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

    @Command
    fun nowPlaying(invoke: Invoke) {
        val args = invoke.parseArgs(NowPlayingArgs::class.java)
        val meta = MediaMetadata.Builder()
            .setTitle(args.title)
            .setArtist(args.artist)
            .setAlbumTitle(args.album)
            .setArtworkUri(args.artworkUrl?.let(Uri::parse))
            .build()
        Playback.onMain {
            Playback.setNowPlaying(meta)
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
}
