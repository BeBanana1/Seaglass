package dev.seaglass.mobile

import android.Manifest
import android.app.Activity
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Build
import androidx.core.app.NotificationChannelCompat
import androidx.core.app.NotificationCompat
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject

/**
 * Nouvelles sorties des artistes suivis (#435), sur un canal à part que l'utilisateur peut
 * couper dans les réglages du système. Rust choisit quoi notifier et envoie le texte et le
 * lien (`https://tidal.com/album/<id>`, ou `seaglass://feed` pour plusieurs sorties) : un
 * appui l'ouvre par `ShareActivity`, comme un lien partagé. Une seule notification à la
 * fois : la suivante la remplace. Classes `…Compat` : les canaux n'existent qu'à partir
 * d'Android 8 (API 26), `minSdk` est à 24.
 */
object ReleaseNotifications {
    private const val CHANNEL = "releases"
    private const val NOTIFICATION_ID = 435
    private const val PERMISSION_REQUEST = 435

    fun show(context: Context, args: JSONObject) {
        val manager = NotificationManagerCompat.from(context)
        // Recréer un canal existant ne fait que mettre son nom à jour (langue de l'app).
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(CHANNEL, NotificationManagerCompat.IMPORTANCE_DEFAULT)
                .setName(args.getString("channel"))
                .build(),
        )
        // Permission refusée (Android 13+) ou notifications de l'app coupées.
        if (!manager.areNotificationsEnabled()) {
            throw IllegalStateException("notifications de l'app désactivées")
        }
        val intent = Intent(Intent.ACTION_VIEW, Uri.parse(args.getString("link")))
            .setClass(context, ShareActivity::class.java)
            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
        val pending = PendingIntent.getActivity(
            context,
            0,
            intent,
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )
        val notification = NotificationCompat.Builder(context, CHANNEL)
            .setSmallIcon(R.drawable.notification_release)
            .setContentTitle(args.getString("title"))
            .setContentText(args.getString("body"))
            .setContentIntent(pending)
            .setAutoCancel(true)
            .setCategory(NotificationCompat.CATEGORY_RECOMMENDATION)
            .build()
        try {
            manager.notify(NOTIFICATION_ID, notification)
        } catch (e: SecurityException) {
            throw IllegalStateException("permission des notifications refusée")
        }
    }

    /** Android 13+ : demande la permission à l'activation du réglage ; sinon rien à faire. */
    fun requestPermission(activity: Activity) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val permission = Manifest.permission.POST_NOTIFICATIONS
        if (activity.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED) return
        activity.requestPermissions(arrayOf(permission), PERMISSION_REQUEST)
    }
}
