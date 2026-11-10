package dev.seaglass.mobile

import android.app.Activity
import android.os.Bundle

/**
 * Mode conduite (#193) : cible du lien `seaglass://drive` et du raccourci de l'app (appui long
 * sur l'icône), que les Modes et routines de Samsung peuvent ouvrir à la connexion de
 * l'autoradio. Sans interface, comme `NowPlayingActivity` : la demande part vers Rust (gardée
 * par `Playback` si l'app n'était pas lancée), l'app revient au premier plan, puis
 * l'activité se ferme.
 */
class DriveActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Playback.onMain { Playback.openDrive() }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
