package dev.seaglass.mobile

import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.net.ConnectivityManager
import android.os.BatteryManager
import android.os.PowerManager
import org.json.JSONObject

/**
 * Réseau et batterie au moment où Rust résout un titre (#365) : la qualité demandée en
 * dépend. Lu à la demande par `Bridge`, sans écouteur permanent : un changement compte à
 * partir du titre suivant.
 */
object DeviceConditions {
    fun read(context: Context): JSONObject {
        val result = JSONObject()
        // Réseau facturé à l'usage (données mobiles, partage de connexion) ou aucun réseau :
        // la pratique d'Android pour « éviter les gros transferts ».
        val connectivity = context.getSystemService(ConnectivityManager::class.java)
        result.put("metered", connectivity?.isActiveNetworkMetered ?: true)
        // Intent collant : lu sans enregistrer de récepteur.
        val battery = context.registerReceiver(null, IntentFilter(Intent.ACTION_BATTERY_CHANGED))
        if (battery != null) {
            val level = battery.getIntExtra(BatteryManager.EXTRA_LEVEL, -1)
            val scale = battery.getIntExtra(BatteryManager.EXTRA_SCALE, -1)
            if (level >= 0 && scale > 0) result.put("battery", level * 100 / scale)
            // Branché, même batterie pleine : rien à ménager.
            result.put("charging", battery.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0)
        }
        val power = context.getSystemService(PowerManager::class.java)
        result.put("powerSave", power?.isPowerSaveMode ?: false)
        return result
    }
}
