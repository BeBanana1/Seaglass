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
 * Crashes on the Kotlin side (#192), in the logs folder chosen by Rust (`logs/`).
 *
 * - Uncaught exception: its stack goes into `crash.txt`, which Rust attaches to the report
 *   at the next launch, then Android's handler takes over (the app closes).
 * - Native crash (Rust panic included) and ANR: Android 11+ keeps the reason for the last
 *   process exits; the most recent one not yet seen is returned to Rust.
 */
object CrashLog {
    /** Same name as `report::CRASH_FILE` on the Rust side. */
    private const val CRASH_FILE = "crash.txt"
    /** Date of the last exit already examined: each exit is reported only once. */
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
                    "${header()}\nuncaught Kotlin exception in thread \"${thread.name}\"\n\n" +
                        error.stackTraceToString(),
                )
            } catch (_: Throwable) {
                // Nothing more to do: the crash runs its course.
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
            // At worst, the same exit is reported again at the next launch.
        }
        val exit = exits.firstOrNull { it.timestamp > last && kind(it.reason) != null } ?: return null
        return describe(exit)
    }

    private fun kind(reason: Int): String? = when (reason) {
        ApplicationExitInfo.REASON_CRASH -> "Java/Kotlin exception"
        ApplicationExitInfo.REASON_CRASH_NATIVE -> "native crash"
        ApplicationExitInfo.REASON_ANR -> "application not responding (ANR)"
        else -> null
    }

    private fun describe(exit: ApplicationExitInfo): String {
        val text = StringBuilder()
        text.append("${kind(exit.reason)} on ${utc(exit.timestamp)}")
        exit.description?.let { text.append("\n").append(it) }
        // A native crash trace is a binary tombstone: only an ANR's is read.
        if (exit.reason == ApplicationExitInfo.REASON_ANR) {
            try {
                exit.traceInputStream?.use { stream ->
                    // `readNBytes` only exists from Android 13.
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
                // Unreadable trace: the reason is enough.
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
