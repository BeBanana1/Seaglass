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
 * Installation d'une mise à jour (#24) : l'APK, déjà téléchargé et vérifié par Rust
 * (SHA-256 du manifeste), passe par une session `PackageInstaller`. Android affiche sa
 * boîte de confirmation et refuse un APK signé par une autre clé que l'app installée.
 */
object ApkInstaller {
    private const val TAG = "SeaglassUpdate"
    private const val ACTION = "dev.seaglass.mobile.INSTALL_STATUS"

    /** Message reconnu par Rust : Seaglass n'a pas encore le droit d'installer des apps. */
    const val NEEDS_PERMISSION = "install-permission"

    private var receiver: BroadcastReceiver? = null
    /** Réponse en attente : appelée une fois, à la confirmation affichée ou à l'échec. */
    private var pending: ((String?) -> Unit)? = null

    /**
     * `done(null)` quand la boîte de confirmation est affichée, `done(message)` sinon.
     * Appelé sur le thread principal.
     */
    fun install(activity: Activity, path: String, done: (String?) -> Unit) {
        val context = activity.applicationContext
        if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.O &&
            !context.packageManager.canRequestPackageInstalls()
        ) {
            // Réglage « Installer des applis inconnues » de Seaglass ; l'utilisateur réessaie.
            activity.startActivity(
                Intent(Settings.ACTION_MANAGE_UNKNOWN_APP_SOURCES, Uri.parse("package:${context.packageName}"))
            )
            done(NEEDS_PERMISSION)
            return
        }
        register(context)
        pending = done
        val file = File(path)
        // Copie de quelques dizaines de Mo : hors du thread principal.
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
                Log.w(TAG, "session d'installation impossible", e)
                Playback.onMain { answer("installation impossible : ${e.javaClass.simpleName}") }
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
                            answer("confirmation d'installation absente")
                            return
                        }
                        confirm.addFlags(Intent.FLAG_ACTIVITY_NEW_TASK)
                        ctx.startActivity(confirm)
                        answer(null)
                    }
                    // En cas de succès, Android remplace le processus : rien à faire.
                    PackageInstaller.STATUS_SUCCESS -> Log.i(TAG, "mise à jour installée")
                    else -> {
                        val message = intent.getStringExtra(PackageInstaller.EXTRA_STATUS_MESSAGE)
                        Log.w(TAG, "installation refusée ($status) : $message")
                        if (pending != null) {
                            answer("installation refusée ($status)")
                        } else if (status != PackageInstaller.STATUS_FAILURE_ABORTED) {
                            // Après la confirmation : l'UI a déjà sa réponse, on prévient ici.
                            Toast.makeText(ctx, "Seaglass : installation refusée ($status)", Toast.LENGTH_LONG).show()
                        }
                    }
                }
            }
        }
        ContextCompat.registerReceiver(context, r, IntentFilter(ACTION), ContextCompat.RECEIVER_NOT_EXPORTED)
        receiver = r
    }
}
