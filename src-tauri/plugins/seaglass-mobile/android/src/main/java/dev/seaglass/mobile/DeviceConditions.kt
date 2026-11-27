package dev.seaglass.mobile

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.os.BatteryManager
import android.os.PowerManager
import org.json.JSONObject

/**
 * Network and battery when Rust resolves a track (#365): the requested quality depends on
 * them. Read on demand by `Bridge`, with no permanent listener: a change counts from the
 * next track.
 */
object DeviceConditions {
    fun read(context: Context): JSONObject {
        val result = JSONObject()
        // Metered network (mobile data, tethering) or no network: Android's practice for
        // "avoid large transfers".
        val connectivity = context.getSystemService(ConnectivityManager::class.java)
        result.put("metered", connectivity?.isActiveNetworkMetered ?: true)
        // Sticky intent: read without registering a receiver.
        val battery = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        if (battery != null) {
            val level = battery.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
            val scale = battery.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
            if (level >= 0 && scale > 0) result.put("battery", level * 100 / scale)
            // Plugged in, even with a full battery: nothing to spare.
            result.put("charging", battery.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0)
        }
        val power = context.getSystemService(PowerManager::class.java)
        result.put("powerSave", power?.isPowerSaveMode ?: false)
        return result
    }
}
