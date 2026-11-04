package dev.seaglass.mobile

import android.app.Activity
import android.os.Bundle

/**
 * Cible de l'appui sur la notification (#163). Sans interface : elle demande « En lecture »
 * à Rust (qui le dit à l'UI, ou le garde jusqu'à ce que la WebView soit prête), ramène l'app
 * au premier plan comme le lanceur, puis se ferme. L'activité principale est générée par
 * Tauri : passer par ici évite d'y toucher.
 */
class NowPlayingActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Playback.onMain { Playback.command("openNowPlaying") }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
