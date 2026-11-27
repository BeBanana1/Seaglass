package dev.seaglass.mobile

import android.app.Activity
import android.os.Bundle

/**
 * Drive mode (#193): target of the `seaglass://drive` link and of the app shortcut (long
 * press on the icon), which Samsung's Modes and Routines can open when the car stereo
 * connects. No UI, like `NowPlayingActivity`: the request goes to Rust (held by `Playback`
 * if the app was not running), the app comes back to the foreground, then the activity
 * closes.
 */
class DriveActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        Playback.onMain { Playback.openDrive() }
        packageManager.getLaunchIntentForPackage(packageName)?.let(::startActivity)
        finish()
    }
}
