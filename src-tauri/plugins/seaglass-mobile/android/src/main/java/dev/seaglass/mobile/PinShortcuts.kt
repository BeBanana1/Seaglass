package dev.seaglass.mobile

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.util.Log
import androidx.core.content.pm.ShortcutInfoCompat
import androidx.core.content.pm.ShortcutManagerCompat
import androidx.core.graphics.drawable.IconCompat
import org.json.JSONObject

/**
 * Pins as app icon shortcuts (#335, long press on the icon): the first pins become
 * dynamic shortcuts, which can also be placed on the home screen. A tap opens
 * `PinActivity` with the `seaglass://pin/<key>` link.
 *
 * Rust sends the whole list (`pins`: `key`, `label`, in order) on every change and at
 * startup; nothing is kept here.
 */
object PinShortcuts {
    private const val TAG = "SeaglassShortcuts"
    private const val PREFIX = "pin-"

    fun set(context: Context, args: JSONObject) {
        val pins = args.getJSONArray("pins")
        val max = args.optInt("max", 4)
        val all = ArrayList<ShortcutInfoCompat>()
        for (i in 0 until pins.length()) {
            val pin = pins.getJSONObject(i)
            all.add(shortcut(context, pin.getString("key"), pin.getString("label"), i))
        }
        val ids = all.map { it.id }.toSet()
        try {
            // Pin removed: its shortcut disappears from the icon, and the one placed on the home
            // screen (which the app cannot remove) is disabled with a message.
            val known = ShortcutManagerCompat.getShortcuts(
                context,
                ShortcutManagerCompat.FLAG_MATCH_DYNAMIC or ShortcutManagerCompat.FLAG_MATCH_PINNED,
            )
            val gone = known.map { it.id }.filter { it.startsWith(PREFIX) && it !in ids }
            if (gone.isNotEmpty()) {
                ShortcutManagerCompat.disableShortcuts(context, gone, args.optString("removed"))
                ShortcutManagerCompat.removeDynamicShortcuts(context, gone)
            }
            // Pin back, renamed or dropped out of the first ones: its shortcut placed on the
            // home screen stays valid and follows its name.
            ShortcutManagerCompat.enableShortcuts(context, all)
            ShortcutManagerCompat.updateShortcuts(context, all)
            ShortcutManagerCompat.setDynamicShortcuts(context, all.take(max))
        } catch (e: Exception) {
            // System call rate limit, locked device: the list will be sent again at the next
            // change or the next startup.
            Log.w(TAG, "shortcuts not updated: ${e.javaClass.simpleName}")
        }
    }

    private fun shortcut(context: Context, key: String, label: String, rank: Int): ShortcutInfoCompat {
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse("seaglass://pin/$key"))
            .setClass(context, PinActivity::class.java)
        return ShortcutInfoCompat.Builder(context, PREFIX + key)
            .setShortLabel(label)
            .setLongLabel(label)
            .setIcon(IconCompat.createWithResource(context, context.applicationInfo.icon))
            .setIntent(intent)
            .setRank(rank)
            .build()
    }
}
