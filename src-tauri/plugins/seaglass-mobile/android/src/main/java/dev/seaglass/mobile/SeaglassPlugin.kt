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

@TauriPlugin
class SeaglassPlugin(private val activity: Activity) : Plugin(activity) {
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
