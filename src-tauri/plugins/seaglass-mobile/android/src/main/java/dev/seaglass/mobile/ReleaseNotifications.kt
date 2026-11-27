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
 * New releases from followed artists (#435), on a separate channel the user can turn off
 * in the system settings. Rust chooses what to notify and sends the text and the link
 * (`https://tidal.com/album/<id>`, or `seaglass://feed` for several releases): a tap opens
 * it through `ShareActivity`, like a shared link. One notification at a time: the next
 * one replaces it. `...Compat` classes: channels only exist from Android 8 (API 26),
 * `minSdk` is 24.
 */
object ReleaseNotifications {
    private const val CHANNEL = "releases"
    private const val NOTIFICATION_ID = 435
    private const val PERMISSION_REQUEST = 435

    fun show(context: Context, args: JSONObject) {
        val manager = NotificationManagerCompat.from(context)
        // Recreating an existing channel only updates its name (app language).
        manager.createNotificationChannel(
            NotificationChannelCompat.Builder(CHANNEL, NotificationManagerCompat.IMPORTANCE_DEFAULT)
                .setName(args.getString("channel"))
                .build(),
        )
        // Permission denied (Android 13+) or the app's notifications turned off.
        if (!manager.areNotificationsEnabled()) {
            throw IllegalStateException("app notifications disabled")
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
            throw IllegalStateException("notification permission denied")
        }
    }

    /** Android 13+: asks for the permission when the setting is enabled; else nothing to do. */
    fun requestPermission(activity: Activity) {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.TIRAMISU) return
        val permission = Manifest.permission.POST_NOTIFICATIONS
        if (activity.checkSelfPermission(permission) == PackageManager.PERMISSION_GRANTED) return
        activity.requestPermissions(arrayOf(permission), PERMISSION_REQUEST)
    }
}
