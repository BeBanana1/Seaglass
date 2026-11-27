package dev.seaglass.mobile

import android.app.Activity
import android.os.Bundle

/**
 * Pin to play (#335): target of the icon shortcuts (`PinShortcuts`) and of the
 * `seaglass://pin/<key>` link, which Samsung's Modes and Routines can open. No UI, like
 * `DriveActivity`: the key goes to Rust (held by `Playback` if the app was not running),
 * which validates it; the app comes back to the foreground, then the activity closes.
 */
class PinActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val key = intent?.data
            ?.takeIf { it.scheme == "seaglass" && it.host == "pin" }
            ?.lastPathSegment
        if (key != null) Playback.onMain { LaunchRequests.openPin(key) }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
