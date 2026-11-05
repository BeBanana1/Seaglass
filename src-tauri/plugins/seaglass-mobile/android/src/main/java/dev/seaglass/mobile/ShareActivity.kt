package dev.seaglass.mobile

import android.app.Activity
import android.content.Intent
import android.os.Bundle

/**
 * Cible du menu « Partager » du système (#203) : un lien TIDAL partagé depuis l'app TIDAL ou
 * un navigateur. Sans interface, comme `NowPlayingActivity` : le texte part vers Rust (qui
 * n'en garde que le type et l'identifiant validé), l'app revient au premier plan, puis
 * l'activité se ferme. Si l'app n'était pas lancée, le texte attend que Rust branche la
 * session (`Playback.attachSession`).
 */
class ShareActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val text = intent?.takeIf { it.action == Intent.ACTION_SEND }
            ?.getStringExtra(Intent.EXTRA_TEXT)
            // Rust refuse aussi au-delà de 2 000 caractères : inutile d'en transmettre plus.
            ?.take(4000)
        if (!text.isNullOrBlank()) {
            Playback.onMain { Playback.share(text) }
        }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
