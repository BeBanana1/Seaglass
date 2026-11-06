package dev.seaglass.mobile

import android.content.Context
import android.net.Uri
import android.os.Looper
import androidx.media3.common.MediaMetadata
import org.json.JSONObject
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Commandes que Rust appelle directement en JNI (#169), sans passer par l'activité.
 *
 * Tauri exécute une commande de plugin dans le `PluginManager` de l'activité : quand l'app
 * est balayée des récents, il n'y en a plus et l'appel panique dans wry, alors que le
 * lecteur et la session média continuent. Tout ce que le lecteur, la session et les
 * secrets demandent passe donc par ici ; `Playback` et `SecretStore` vivent avec le
 * processus.
 *
 * Arguments et réponse en JSON : `{"ok": {…}}` ou `{"error": "…"}`. Jamais appelé depuis le
 * thread principal par Rust, mais toléré (exécution directe).
 */
object Bridge {
    /** Le thread principal peut être occupé, jamais bloqué : au-delà, la commande échoue. */
    private const val MAIN_TIMEOUT_S = 10L

    @Volatile
    private var secrets: SecretStore? = null

    @Volatile
    private var context: Context? = null

    /** Contexte de l'application, posé par Rust à la première commande. */
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
        // Messages sans valeur ni détail : seulement le type de l'erreur.
        "secretGet" -> secret("lecture du secret impossible") { store ->
            JSONObject().apply { store.get(args.getString("name"))?.let { put("value", it) } }
        }
        "secretSet" -> secret("écriture du secret impossible") { store ->
            val value = args.string("value") ?: throw IllegalArgumentException("valeur manquante")
            store.set(args.getString("name"), value)
            JSONObject()
        }
        "secretDelete" -> secret("effacement du secret impossible") { store ->
            JSONObject().put("existed", store.delete(args.getString("name")))
        }
        "linkHandling" -> LinkHandling.state(appContext())
        "openLinkSettings" -> {
            LinkHandling.openSettings(appContext())
            null
        }
        else -> throw IllegalArgumentException("commande inconnue : $command")
    }

    private fun appContext(): Context =
        context ?: throw IllegalStateException("pont natif non initialisé")

    private fun secret(failure: String, block: (SecretStore) -> JSONObject): JSONObject {
        val store = secrets ?: throw IllegalStateException("pont natif non initialisé")
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
    }

    /** Exécute `block` sur le thread principal (celui de l'ExoPlayer) et attend sa fin. */
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
            throw IllegalStateException("thread principal indisponible")
        }
        error?.let { throw it }
    }

    // `isNull` couvre aussi la clé absente ; `optString` rendrait "null" pour un null JSON.
    private fun JSONObject.string(key: String): String? = if (isNull(key)) null else getString(key)
    private fun JSONObject.double(key: String): Double? = if (isNull(key)) null else getDouble(key)
    private fun JSONObject.bool(key: String): Boolean? = if (isNull(key)) null else getBoolean(key)
}
