package dev.seaglass.mobile

import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject

/**
 * Requests that open the app on something (#45, moved out of `Playback.kt`): drive mode
 * (#193), a pin (#335), a shared link (#203). They go to Rust through the session's command
 * channel; one received before Rust attached the session (app launched by it) waits until
 * `attach`. Main thread only, like `Playback`.
 */
internal object LaunchRequests {
    private var channel: Channel? = null

    /** Shared text received before Rust plugged in the session (app launched by the share). */
    private var pendingShare: String? = null

    /** Drive mode requested before Rust plugged in the session (app launched by the link). */
    private var pendingDrive = false

    /** Pin requested before Rust plugged in the session (app launched by the shortcut). */
    private var pendingPin: String? = null

    /** The session's command channel (`Playback.attachSession`): sends what was waiting. */
    fun attach(commands: Channel) {
        channel = commands
        pendingPin?.let {
            pendingPin = null
            openPin(it)
        }
        pendingShare?.let {
            pendingShare = null
            share(it)
        }
        if (pendingDrive) {
            pendingDrive = false
            openDrive()
        }
    }

    /** `seaglass://drive` link or app shortcut (#193), on the main thread. */
    fun openDrive() {
        val channel = channel
        if (channel == null) {
            pendingDrive = true
            return
        }
        val o = JSObject()
        o.put("command", "openDrive")
        channel.send(o)
    }

    /** Pin shortcut or `seaglass://pin/<key>` link (#335), on the main thread. */
    fun openPin(key: String) {
        val channel = channel
        if (channel == null) {
            pendingPin = key
            return
        }
        val o = JSObject()
        o.put("command", "openPin")
        o.put("pin", key)
        channel.send(o)
    }

    /** Link received through the "Share" menu (#203), on the main thread. */
    fun share(text: String) {
        val channel = channel
        if (channel == null) {
            pendingShare = text
            return
        }
        val o = JSObject()
        o.put("command", "openLink")
        o.put("text", text)
        channel.send(o)
    }
}
