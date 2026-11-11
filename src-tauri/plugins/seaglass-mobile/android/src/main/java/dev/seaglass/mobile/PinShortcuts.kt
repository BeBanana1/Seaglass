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
 * Épingles en raccourcis de l'icône de l'app (#335, appui long sur l'icône) : les premières
 * épingles deviennent des raccourcis dynamiques, qu'on peut aussi poser sur l'écran d'accueil.
 * Un appui ouvre `PinActivity` avec le lien `seaglass://pin/<clé>`.
 *
 * Rust envoie toute la liste (`pins` : `key`, `label`, dans l'ordre) à chaque changement et au
 * démarrage ; rien n'est gardé ici.
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
            // Épingle retirée : son raccourci disparaît de l'icône, et celui posé sur l'écran
            // d'accueil (que l'app ne peut pas retirer) est désactivé avec un message.
            val known = ShortcutManagerCompat.getShortcuts(
                context,
                ShortcutManagerCompat.FLAG_MATCH_DYNAMIC or ShortcutManagerCompat.FLAG_MATCH_PINNED,
            )
            val gone = known.map { it.id }.filter { it.startsWith(PREFIX) && it !in ids }
            if (gone.isNotEmpty()) {
                ShortcutManagerCompat.disableShortcuts(context, gone, args.optString("removed"))
                ShortcutManagerCompat.removeDynamicShortcuts(context, gone)
            }
            // Épingle revenue, renommée ou sortie des premières : son raccourci posé sur
            // l'écran d'accueil reste valable et suit son nom.
            ShortcutManagerCompat.enableShortcuts(context, all)
            ShortcutManagerCompat.updateShortcuts(context, all)
            ShortcutManagerCompat.setDynamicShortcuts(context, all.take(max))
        } catch (e: Exception) {
            // Limite d'appels du système, appareil verrouillé : la liste repartira au
            // prochain changement ou au prochain démarrage.
            Log.w(TAG, "raccourcis non mis à jour : ${e.javaClass.simpleName}")
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
