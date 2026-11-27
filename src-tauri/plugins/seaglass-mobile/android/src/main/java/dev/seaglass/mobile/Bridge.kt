package dev.seaglass.mobile

import android.content.Context
import android.net.Uri
import android.os.Looper
import androidx.media3.common.MediaMetadata
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Commands that Rust calls directly over JNI (#169), without going through the activity.
 *
 * Tauri runs a plugin command in the activity's `PluginManager`: when the app is swiped
 * away from recents, there is none left and the call panics in wry, while the player and
 * the media session keep going. Everything the player, the session and the secrets ask for
 * therefore goes through here; `Playback` and `SecretStore` live with the process.
 *
 * Arguments and response in JSON: `{"ok": {…}}` or `{"error": "…"}`. Never called from the
 * main thread by Rust, but tolerated (direct execution).
 */
object Bridge {
    /** The main thread may be busy, never blocked: beyond this, the command fails. */
    private const val MAIN_TIMEOUT_S = 10L

    @Volatile
    private var secrets: SecretStore? = null

    @Volatile
    private var context: Context? = null

    /** The application's context, set by Rust at the first command. */
    @JvmStatic
    fun init(context: Context) {
        synchronized(this) {
            if (secrets == null) secrets = SecretStore(context.applicationContext)
            if (this.context == null) this.context = context.applicationContext
        }
    }

    @JvmStatic
    fun call(command: String, args: String): String {
        val response = JSONObject()
        try {
            response.put("ok", run(command, JSONObject(args)) ?: JSONObject())
        } catch (e: Exception) {
            response.put("error", e.message ?: e.javaClass.simpleName)
        }
        return response.toString()
    }

    private fun run(command: String, args: JSONObject): JSONObject? = when (command) {
        "engine" -> {
            val engine = engineArgs(args)
            onMainSync { Playback.apply(engine) }
            null
        }
        "nowPlaying" -> {
            val meta = MediaMetadata.Builder()
                .setTitle(args.string("title"))
                .setArtist(args.string("artist"))
                .setAlbumTitle(args.string("album"))
                .setArtworkUri(args.string("artworkUrl")?.let(Uri::parse))
                .build()
            onMainSync { Playback.setNowPlaying(meta) }
            null
        }
        "sessionModes" -> {
            val modes = SessionModes(
                shuffle = args.optBoolean("shuffle", false),
                repeat = args.string("repeat") ?: "off",
                shuffleLabel = args.string("shuffleLabel") ?: "Shuffle",
                repeatLabel = args.string("repeatLabel") ?: "Repeat",
            )
            onMainSync { Playback.setModes(modes) }
            null
        }
        // Messages without value or detail: only the type of the error.
        "secretGet" -> secret("could not read the secret") { store ->
            JSONObject().apply { store.get(args.getString("name"))?.let { put("value", it) } }
        }
        "secretSet" -> secret("could not write the secret") { store ->
            val value = args.string("value") ?: throw IllegalArgumentException("missing value")
            store.set(args.getString("name"), value)
            JSONObject()
        }
        "secretDelete" -> secret("could not delete the secret") { store ->
            JSONObject().put("existed", store.delete(args.getString("name")))
        }
        "crashSetup" -> CrashLog.setup(appContext(), java.io.File(args.getString("dir")))
        "linkHandling" -> LinkHandling.state(appContext())
        "deviceConditions" -> DeviceConditions.read(appContext())
        "pinShortcuts" -> {
            PinShortcuts.set(appContext(), args)
            null
        }
        "notifyRelease" -> {
            ReleaseNotifications.show(appContext(), args)
            null
        }
        "openLinkSettings" -> {
            LinkHandling.openSettings(appContext())
            null
        }
        else -> throw IllegalArgumentException("unknown command: $command")
    }

    private fun appContext(): Context =
        context ?: throw IllegalStateException("native bridge not initialized")

    private fun secret(failure: String, block: (SecretStore) -> JSONObject): JSONObject {
        val store = secrets ?: throw IllegalStateException("native bridge not initialized")
        return try {
            synchronized(store) { block(store) }
        } catch (e: Exception) {
            throw IllegalStateException("$failure : ${e.javaClass.simpleName}")
        }
    }

    private fun engineArgs(args: JSONObject) = EngineArgs().apply {
        op = args.getString("op")
        url = args.string("url")
        start = args.double("start")
        gain = args.double("gain")
        paused = args.bool("paused")
        position = args.double("position")
        on = args.bool("on")
        level = if (args.isNull("level")) null else args.getInt("level")
        muted = args.bool("muted")
        eq = if (args.isNull("eq")) null else eqSpec(args.getJSONObject("eq"))
    }

    /** Equalizer profile (#467): `{preamp, filters: [{kind, freq, gain, q}]}`. */
    private fun eqSpec(o: JSONObject): EqSpec {
        val list = o.getJSONArray("filters")
        val filters = (0 until list.length()).map {
            val f = list.getJSONObject(it)
            EqFilterSpec(f.getString("kind"), f.getDouble("freq"), f.getDouble("gain"), f.getDouble("q"))
        }
        return EqSpec(o.getDouble("preamp"), filters)
    }

    /** Runs `block` on the main thread (ExoPlayer's) and waits for it to finish. */
    private fun onMainSync(block: () -> Unit) {
        if (Looper.myLooper() == Looper.getMainLooper()) return block()
        val done = CountDownLatch(1)
        var error: Throwable? = null
        Playback.onMain {
            try {
                block()
            } catch (e: Throwable) {
                error = e
            } finally {
                done.countDown()
            }
        }
        if (!done.await(MAIN_TIMEOUT_S, TimeUnit.SECONDS)) {
            throw IllegalStateException("main thread unavailable")
        }
        error?.let { throw it }
    }

    // `isNull` also covers a missing key; `optString` would return "null" for a JSON null.
    private fun JSONObject.string(key: String): String? = if (isNull(key)) null else getString(key)
    private fun JSONObject.double(key: String): Double? = if (isNull(key)) null else getDouble(key)
    private fun JSONObject.bool(key: String): Boolean? = if (isNull(key)) null else getBoolean(key)
}
