package dev.seaglass.mobile

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Target of the system "Share" menu (#203): a TIDAL link shared from the TIDAL app or a
 * browser; also a tapped `https://tidal.com/…` link, if the user gave these links to the
 * app (`LinkHandling`). No UI, like `NowPlayingActivity`: the text goes to Rust (which
 * only keeps the type and the validated identifier), the app comes back to the
 * foreground, then the activity closes. If the app was not running, the text waits until
 * Rust attaches the session (`Playback.attachSession`).
 */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = when (intent?.action) {
            Intent.ACTION_SEND -> intent.getStringExtra(Intent.EXTRA_TEXT)
            Intent.ACTION_VIEW -> intent.dataString
            else -> null
        }
            // Rust also rejects anything beyond 2,000 characters: no need to pass more.
            ?.take(4000)
        if (!text.isNullOrBlank()) {
            Playback.onMain { Playback.share(text) }
        }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
