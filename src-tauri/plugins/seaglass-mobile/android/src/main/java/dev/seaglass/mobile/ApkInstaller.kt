package dev.seaglass.mobile

import android.app.Activity
import android.app.PendingIntent
import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.content.pm.PackageInstaller
import android.net.Uri
import android.os.Build
import android.provider.Settings
import android.util.Log
import android.widget.Toast
import androidx.core.content.ContextCompat
import java.io.File

/**
 * Installing an update (#24): the APK, already downloaded and verified by Rust (SHA-256
 * from the manifest), goes through a `PackageInstaller` session. Android shows its
 * confirmation dialog and rejects an APK signed with a different key than the installed app.
 */
object ApkInstaller {
    private const val TAG = "SeaglassUpdate"
    private const val ACTION = "dev.seaglass.mobile.INSTALL_STATUS"

    /** Message recognized by Rust: Seaglass is not yet allowed to install apps. */
    const val NEEDS_PERMISSION = "install-permission"

    private var receiver: BroadcastReceiver? = null
    /** Pending answer: called once, when the confirmation is shown or on failure. */
    private var pending: ((String?) -> Unit)? = null

    /**
     * `done(null)` when the confirmation dialog is shown, `done(message)` otherwise.
     * Called on the main thread.
     */
    fun install(activity: Activity, path: String, done: (String?) -> Unit) {
        val context = activity.applicationContext
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !context.packageManager.canRequestPackageInstalls()
        ) {
            // Seaglass's "Install unknown apps" setting; the user retries.
            activity.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
            )
            done(NEEDS_PERMISSION)
            return
        }
        register(context)
        pending = done
        val file = File(path)
        // Copying a few tens of MB: off the main thread.
        Thread {
            try {
                val installer = context.packageManager.packageInstaller
                val params = PackageInstaller.SessionParams(PackageInstaller.SessionParams.MODE_FULL_INSTALL)
                params.setAppPackageName(context.packageName)
                val id = installer.createSession(params)
                installer.openSession(id).use { session ->
                    session.openWrite("seaglass.apk", 0, file.length()).use { out ->
                        file.inputStream().use { it.copyTo(out) }
                        session.fsync(out)
                    }
                    val intent = Intent(ACTION).setPackage(context.packageName)
                    val flags = PendingIntent.FLAG_UPDATE_CURRENT or
                        (if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) PendingIntent.FLAG_MUTABLE else 0)
                    session.commit(PendingIntent.getBroadcast(context, id, intent, flags).intentSender)
                }
            } catch (e: Exception) {
                Log.w(TAG, "cannot create the install session", e)
                Playback.onMain { answer("install failed: ${e.javaClass.simpleName}") }
            } finally {
                file.delete()
            }
        }.start()
    }

    private fun answer(message: String?) {
        pending?.invoke(message)
        pending = null
    }

    private fun register(context: Context) {
        if (receiver != null) return
        val r = object : BroadcastReceiver() {
            override fun onReceive(ctx: Context, intent: Intent) {
                val status = intent.getIntExtra(PackageInstaller.EXTRA_STATUS, PackageInstaller.STATUS_FAILURE)
                when (status) {
                    PackageInstaller.STATUS_PENDING_USER_ACTION -> {
                        @Suppress("DEPRECATION")
                        val confirm = intent.getParcelableExtra<Intent>(Intent.EXTRA_INTENT)
                        if (confirm == null) {
                            answer("install confirmation missing")
                            return
                        }
                        confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        ctx.startActivity(confirm)
                        answer(null)
                    }
                    // On success, Android replaces the process: nothing to do.
                    PackageInstaller.STATUS_SUCCESS -> Log.i(TAG, "update installed")
                    else -> {
                        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
                        Log.w(TAG, "install rejected ($status): $message")
                        if (pending != null) {
                            answer("install rejected ($status)")
                        } else if (status != PackageInstaller.STATUS_FAILURE_ABORTED) {
                            // After the confirmation: the UI already has its answer, so warn here.
                            Toast.makeText(ctx, ctx.getString(R.string.install_rejected, status), Toast.LENGTH_LONG).show()
                        }
                    }
                }
            }
        }
        ContextCompat.registerReceiver(context, r, IntentFilter(ACTION), ContextCompat.RECEIVER_NOT_EXPORTED)
        receiver = r
    }
}
