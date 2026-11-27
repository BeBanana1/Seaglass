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

/** An operation of `engine::Op` (fields depend on `op`), read by `Bridge`. */
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
    /** Equalizer (#467) with `play` and `currentGain`; `null` = disabled. */
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
 * Commands that need the activity, or that only run at launch (event channels). The ones
 * that the player, the media session and the secrets send along the way go through
 * `Bridge` (#169): the app may then have been swiped away from recents.
 */
@TauriPlugin
class SeaglassPlugin(private val activity: Activity) : Plugin(activity) {
    /** The app's WebView: the keyboard height is passed to it (#251). */
    override fun load(webView: WebView) {
        KeyboardInset.watch(webView)
    }

    /** Installs the update APK already verified by Rust (`apk_update.rs`). */
    @Command
    fun installApk(invoke: Invoke) {
        val args = invoke.parseArgs(InstallArgs::class.java)
        Playback.onMain {
            ApkInstaller.install(activity, args.path) { error ->
                if (error == null) invoke.resolve() else invoke.reject(error)
            }
        }
    }

    /** Drive mode (#193): the screen stays on while it is open. */
    @Command
    fun keepScreenOn(invoke: Invoke) {
        val args = invoke.parseArgs(ScreenArgs::class.java)
        activity.runOnUiThread {
            val flag = WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON
            if (args.on) activity.window.addFlags(flag) else activity.window.clearFlags(flag)
            invoke.resolve()
        }
    }

    /** Notifications for new releases turned on (#435): Android 13+ permission. */
    @Command
    fun requestNotifications(invoke: Invoke) {
        activity.runOnUiThread {
            ReleaseNotifications.requestPermission(activity)
            invoke.resolve()
        }
    }

    /** Creates the player (once per process) and sends its events back on `channel`. */
    @Command
    fun engineStart(invoke: Invoke) {
        val args = invoke.parseArgs(ChannelArgs::class.java)
        val context = activity.applicationContext
        Playback.onMain {
            Playback.start(context, args.channel)
            invoke.resolve()
        }
    }

    /** Media session actions (notification, headphones), sent back on `channel`. */
    @Command
    fun sessionStart(invoke: Invoke) {
        val args = invoke.parseArgs(ChannelArgs::class.java)
        Playback.onMain {
            Playback.attachSession(args.channel)
            invoke.resolve()
        }
    }

    /**
     * TIDAL sign-in (#24): Android has only one WebView for the app, so the sign-in page
     * opens in a separate WebView, without Tauri IPC. The first navigation to the redirect
     * is blocked and its URL handed back to Rust, which extracts the PKCE code from it.
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
            // Web pages only (#145): no device files (true by default before Android 11)
            // and no content providers.
            webView.settings.allowFileAccess = false
            webView.settings.allowContentAccess = false
            webView.webViewClient = object : WebViewClient() {
                override fun shouldOverrideUrlLoading(view: WebView, request: WebResourceRequest): Boolean {
                    val url = request.url.toString()
                    if (url.startsWith(args.redirectPrefix)) {
                        finish(url)
                        return true
                    }
                    // Any other scheme (`intent:`, `file:`, `javascript:`…) is blocked.
                    return request.url.scheme != "https"
                }

                // Safety net: a redirect that does not go through shouldOverrideUrlLoading.
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
     * Sign-in page data (#258), before and after use: TIDAL's web session must not survive,
     * and a new sign-in may pick another account. `CookieManager` is shared by the whole
     * process, but the app's interface has no cookies; web storage is only cleared for
     * TIDAL's origins.
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
