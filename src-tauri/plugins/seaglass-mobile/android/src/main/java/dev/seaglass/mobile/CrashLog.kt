package dev.seaglass.mobile

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import org.json.JSONObject
import java.io.File
import java.text.SimpleDateFormat
import java.util.Date
import java.util.Locale
import java.util.TimeZone

/**
 * Plantages côté Kotlin (#192), dans le dossier des journaux choisi par Rust (`logs/`).
 *
 * - Exception non rattrapée : sa pile va dans `crash.txt`, que Rust joint au rapport au
 *   lancement suivant, puis le gestionnaire d'Android prend le relais (fermeture de l'app).
 * - Plantage natif (panic Rust compris) et ANR : Android 11+ garde la raison des dernières
 *   sorties du processus ; la plus récente pas encore vue est renvoyée à Rust.
 */
object CrashLog {
    /** Même nom que `report::CRASH_FILE` côté Rust. */
    private const val CRASH_FILE = "crash.txt"
    /** Date de la dernière sortie déjà examinée : chaque sortie n'est rapportée qu'une fois. */
    private const val SEEN_FILE = "exit-seen"
    private const val HISTORY = 5
    private const val MAX_TRACE = 64 * 1024

    @Volatile
    private var installed = false

    fun setup(context: Context, dir: File): JSONObject {
        dir.mkdirs()
        install(dir)
        val result = JSONObject()
        lastExit(context, dir)?.let { result.put("exit", it) }
        return result
    }

    private fun install(dir: File) {
        synchronized(this) {
            if (installed) return
            installed = true
        }
        val previous = Thread.getDefaultUncaughtExceptionHandler()
        Thread.setDefaultUncaughtExceptionHandler { thread, error ->
            try {
                File(dir, CRASH_FILE).writeText(
                    "${header()}\nexception Kotlin non rattrapée dans le thread « ${thread.name} »\n\n" +
                        error.stackTraceToString(),
                )
            } catch (_: Throwable) {
                // Rien à faire de plus : le plantage suit son cours.
            }
            previous?.uncaughtException(thread, error)
        }
    }

    private fun lastExit(context: Context, dir: File): String? {
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.R) return null
        val manager = context.getSystemService(ActivityManager::class.java) ?: return null
        val exits = try {
            manager.getHistoricalProcessExitReasons(null, 0, HISTORY)
        } catch (_: Exception) {
            return null
        }
        val newest = exits.firstOrNull() ?: return null
        val seen = File(dir, SEEN_FILE)
        val last = try {
            seen.readText().trim().toLongOrNull() ?: 0L
        } catch (_: Exception) {
            0L
        }
        try {
            seen.writeText(newest.timestamp.toString())
        } catch (_: Exception) {
            // Au pire, la même sortie est rapportée au prochain lancement.
        }
        val exit = exits.firstOrNull { it.timestamp > last && kind(it.reason) != null } ?: return null
        return describe(exit)
    }

    private fun kind(reason: Int): String? = when (reason) {
        ApplicationExitInfo.REASON_CRASH -> "exception Java/Kotlin"
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "plantage natif"
        ApplicationExitInfo.REASON_ANR -> "application figée (ANR)"
        else -> null
    }

    private fun describe(exit: ApplicationExitInfo): String {
        val text = StringBuilder()
        text.append("${kind(exit.reason)} le ${utc(exit.timestamp)}")
        exit.description?.let { text.append("\n").append(it) }
        // La trace d'un plantage natif est un tombstone binaire : seulement celle d'un ANR.
        if (exit.reason == ApplicationExitInfo.REASON_ANR) {
            try {
                exit.traceInputStream?.use { stream ->
                    // `readNBytes` n'existe qu'à partir d'Android 13.
                    val buffer = ByteArray(MAX_TRACE)
                    var size = 0
                    while (size < MAX_TRACE) {
                        val read = stream.read(buffer, size, MAX_TRACE - size)
                        if (read < 0) break
                        size += read
                    }
                    text.append("\n\n").append(String(buffer, 0, size, Charsets.UTF_8))
                }
            } catch (_: Exception) {
                // Trace illisible : la raison suffit.
            }
        }
        return text.toString()
    }

    private fun header(): String =
        "Seaglass (Android ${Build.VERSION.RELEASE}, ${Build.MODEL}), ${utc(System.currentTimeMillis())}"

    private fun utc(millis: Long): String {
        val format = SimpleDateFormat("yyyy-MM-dd HH:mm:ss 'UTC'", Locale.ROOT)
        format.timeZone = TimeZone.getTimeZone("UTC")
        return format.format(Date(millis))
    }
}
