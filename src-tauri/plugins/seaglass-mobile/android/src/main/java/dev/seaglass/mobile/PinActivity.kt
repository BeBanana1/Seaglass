package dev.seaglass.mobile

import android.app.Activity
import android.os.Bundle

/**
 * Épingle à lire (#335) : cible des raccourcis de l'icône (`PinShortcuts`) et du lien
 * `seaglass://pin/<clé>`, que les Modes et routines de Samsung peuvent ouvrir. Sans interface,
 * comme `DriveActivity` : la clé part vers Rust (gardée par `Playback` si l'app n'était pas
 * lancée), qui la valide ; l'app revient au premier plan, puis l'activité se ferme.
 */
class PinActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val key = intent?.data
            ?.takeIf { it.scheme == "seaglass" && it.host == "pin" }
            ?.lastPathSegment
        if (key != null) Playback.onMain { Playback.openPin(key) }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
