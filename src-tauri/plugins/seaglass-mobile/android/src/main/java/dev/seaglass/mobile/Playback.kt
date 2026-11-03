package dev.seaglass.mobile

import android.content.Context
import android.content.Intent
import android.net.Uri
import android.os.Handler
import android.os.Looper
import android.util.Log
import androidx.annotation.OptIn
import androidx.lifecycle.DefaultLifecycleObserver
import androidx.lifecycle.LifecycleOwner
import androidx.lifecycle.ProcessLifecycleOwner
import androidx.media3.common.AudioAttributes
import androidx.media3.common.C
import androidx.media3.common.MediaItem
import androidx.media3.common.MediaMetadata
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.Timeline
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioSink
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject
import kotlin.math.min
import kotlin.math.pow

/**
 * Moteur de lecture Android (#24) : l'ExoPlayer de l'app et sa fenêtre de 2 entrées au
 * plus (titre courant + suivant préchargé), comme mpv sur le bureau. Rust décide de tout
 * (`engine::Op`) ; ce qui se passe ici remonte en événements par `events`.
 *
 * Tout tourne sur le thread principal, celui de l'ExoPlayer. L'objet vit aussi longtemps
 * que le processus : le service et la session peuvent être recréés autour de lui.
 */
@OptIn(UnstableApi::class)
internal object Playback {
    private const val TAG = "SeaglassPlayback"
    /**
     * Position envoyée pendant la lecture (#147) : l'UI interpole entre deux envois et Rust
     * n'en a besoin que pour le préchargement (30 s avant la fin) et la sauvegarde. Plus
     * espacée quand aucune activité n'est visible (écran éteint, autre app au premier plan).
     * Un saut, une pause ou un changement de titre l'envoient toujours aussitôt.
     */
    private const val FOREGROUND_POSITION_MS = 1_000L
    private const val BACKGROUND_POSITION_MS = 5_000L

    private val main = Handler(Looper.getMainLooper())
    private var context: Context? = null
    private var exo: ExoPlayer? = null

    /** Le lecteur vu par la session : ses commandes repartent vers Rust. */
    var remote: RemotePlayer? = null
        private set

    private var events: Channel? = null
    private var commands: Channel? = null

    private var level = 100
    private var muted = false
    private var currentGain: Double? = null
    private var nextGain: Double? = null
    private var metadata: MediaMetadata = MediaMetadata.EMPTY
    private var idle: Boolean? = null
    private var buffering: Boolean? = null
    private var foreground = true
    private var observing = false

    fun onMain(block: () -> Unit) = main.post(block)

    /** Crée le lecteur s'il n'existe pas encore, puis branche les événements sur `channel`. */
    fun start(appContext: Context, channel: Channel) {
        context = appContext
        val player = exo ?: build(appContext)
        events = channel
        idle = null
        buffering = null
        send("driver") { put("value", "audiotrack") }
        updateIdle(player)
        startService()
        if (!observing) {
            ProcessLifecycleOwner.get().lifecycle.addObserver(appLifecycle)
            observing = true
        }
    }

    fun attachSession(channel: Channel) {
        commands = channel
    }

    /** Action de la notification, de l'écran verrouillé ou des écouteurs. */
    fun command(name: String, positionMs: Long? = null) {
        val o = JSObject()
        o.put("command", name)
        if (positionMs != null) o.put("position", positionMs / 1000.0)
        commands?.send(o) ?: Log.w(TAG, "commande « $name » sans session branchée")
    }

    fun apply(args: EngineArgs) {
        val p = exo ?: throw IllegalStateException("lecteur non démarré")
        when (args.op) {
            "play" -> {
                val url = args.url ?: throw IllegalArgumentException("url manquante")
                p.setMediaItem(item(url, metadata), ((args.start ?: 0.0) * 1000).toLong())
                currentGain = args.gain
                nextGain = null
                applyVolume()
                p.prepare()
                p.playWhenReady = true
                startService()
            }
            "preload" -> {
                val url = args.url ?: throw IllegalArgumentException("url manquante")
                p.addMediaItem(item(url, MediaMetadata.EMPTY))
                nextGain = args.gain
            }
            "removeNext" -> {
                val next = p.currentMediaItemIndex + 1
                if (p.mediaItemCount > next) p.removeMediaItems(next, p.mediaItemCount)
                nextGain = null
            }
            "dropPrevious" -> {
                val current = p.currentMediaItemIndex
                if (current > 0) p.removeMediaItems(0, current)
            }
            "stop" -> {
                p.stop()
                p.clearMediaItems()
            }
            "togglePause" -> p.playWhenReady = !p.playWhenReady
            "setPaused" -> p.playWhenReady = args.paused != true
            "seek" -> p.seekTo(((args.position ?: 0.0) * 1000).toLong())
            "loop" -> p.repeatMode = if (args.on == true) Player.REPEAT_MODE_ONE else Player.REPEAT_MODE_OFF
            "volume" -> {
                level = (args.level ?: 100).coerceIn(0, 100)
                muted = args.muted == true
                applyVolume()
            }
            "currentGain" -> {
                currentGain = args.gain
                applyVolume()
            }
            else -> throw IllegalArgumentException("opération inconnue : ${args.op}")
        }
    }

    /** Titre, artiste, album et pochette du titre courant, pour la notification. */
    fun setNowPlaying(meta: MediaMetadata) {
        metadata = meta
        val p = exo ?: return
        if (p.mediaItemCount == 0) return
        val index = p.currentMediaItemIndex
        val current = p.getMediaItemAt(index)
        // Même URI : Media3 met à jour l'entrée sans recharger le flux.
        p.replaceMediaItem(index, current.buildUpon().setMediaMetadata(meta).build())
    }

    private fun build(appContext: Context): ExoPlayer {
        val attributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()
        val player = ExoPlayer.Builder(appContext)
            .setAudioAttributes(attributes, true)
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            .build()
        player.addListener(listener)
        player.addAnalyticsListener(analytics)
        exo = player
        remote = RemotePlayer(player)
        return player
    }

    private fun startService() {
        val ctx = context ?: return
        try {
            ctx.startService(Intent(ctx, PlaybackService::class.java))
        } catch (e: IllegalStateException) {
            // Application en arrière-plan : la session reprendra au prochain lancement.
            Log.w(TAG, "service de lecture non démarré : $e")
        }
    }

    /** Une URL `data:` porte le MPD (rien sur disque ni sur un port local) ; sinon flux direct. */
    private fun item(url: String, meta: MediaMetadata): MediaItem {
        val builder = MediaItem.Builder().setUri(Uri.parse(url)).setMediaMetadata(meta)
        if (url.startsWith("data:application/dash+xml")) builder.setMimeType(MimeTypes.APPLICATION_MPD)
        return builder.build()
    }

    /** Volume de l'app avec la courbe cubique de mpv, et gain de normalisation (≤ 0 dB). */
    private fun applyVolume() {
        val p = exo ?: return
        val gain = currentGain?.let { min(1.0, 10.0.pow(it / 20.0)) } ?: 1.0
        p.volume = if (muted) 0f else ((level / 100.0).pow(3) * gain).toFloat()
    }

    private fun send(type: String, fill: JSObject.() -> Unit = {}) {
        val o = JSObject()
        o.put("type", type)
        o.fill()
        events?.send(o)
    }

    private fun sendPosition() {
        val p = exo ?: return
        send("position") { if (p.mediaItemCount > 0) put("value", p.currentPosition / 1000.0) }
    }

    private fun sendDuration() {
        val p = exo ?: return
        val duration = p.duration
        send("duration") { if (duration != C.TIME_UNSET) put("value", duration / 1000.0) }
    }

    private fun updateIdle(p: Player) {
        val now = p.mediaItemCount == 0 ||
            p.playbackState == Player.STATE_ENDED ||
            p.playbackState == Player.STATE_IDLE
        if (now != idle) {
            idle = now
            send("idle") { put("value", now) }
        }
        val loading = p.playbackState == Player.STATE_BUFFERING && p.playWhenReady
        if (loading != buffering) {
            buffering = loading
            send("buffering") { put("value", loading) }
        }
    }

    private val tick = object : Runnable {
        override fun run() {
            val p = exo ?: return
            sendPosition()
            if (p.isPlaying) {
                main.postDelayed(this, if (foreground) FOREGROUND_POSITION_MS else BACKGROUND_POSITION_MS)
            }
        }
    }

    /** Retour au premier plan : position aussitôt, puis à la cadence du premier plan. */
    private val appLifecycle = object : DefaultLifecycleObserver {
        override fun onStart(owner: LifecycleOwner) {
            foreground = true
            main.removeCallbacks(tick)
            if (exo?.isPlaying == true) main.post(tick) else sendPosition()
        }

        override fun onStop(owner: LifecycleOwner) {
            foreground = false
        }
    }

    private val listener = object : Player.Listener {
        override fun onIsPlayingChanged(isPlaying: Boolean) {
            main.removeCallbacks(tick)
            if (isPlaying) main.post(tick) else sendPosition()
        }

        override fun onPlayWhenReadyChanged(playWhenReady: Boolean, reason: Int) {
            send("paused") { put("value", !playWhenReady) }
            exo?.let { updateIdle(it) }
        }

        override fun onPlaybackStateChanged(state: Int) {
            val p = exo ?: return
            when (state) {
                Player.STATE_READY -> {
                    send("loaded")
                    sendDuration()
                }
                Player.STATE_ENDED -> send("ended") { put("reason", "eof") }
            }
            updateIdle(p)
        }

        override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
            // Seul l'enchaînement automatique vaut passage au préchargé ; remplacer la
            // fenêtre (`play`) ou boucler (repeat One) n'en est pas un.
            if (reason != Player.MEDIA_ITEM_TRANSITION_REASON_AUTO) return
            currentGain = nextGain
            nextGain = null
            applyVolume()
            send("advanced")
            sendDuration()
        }

        override fun onTimelineChanged(timeline: Timeline, reason: Int) = sendDuration()

        override fun onPositionDiscontinuity(
            oldPosition: Player.PositionInfo,
            newPosition: Player.PositionInfo,
            reason: Int,
        ) = sendPosition()

        override fun onPlayerError(error: PlaybackException) {
            val detail = listOfNotNull(error.errorCodeName, error.cause?.message?.let(::redact))
                .joinToString(" : ")
                .take(300)
            Log.w(TAG, "erreur de lecture : $detail")
            send("ended") {
                put("reason", "error")
                put("error", detail)
            }
            exo?.let { updateIdle(it) }
        }
    }

    private val analytics = object : AnalyticsListener {
        override fun onAudioTrackInitialized(
            eventTime: AnalyticsListener.EventTime,
            audioTrackConfig: AudioSink.AudioTrackConfig,
        ) {
            val format = when (audioTrackConfig.encoding) {
                C.ENCODING_PCM_16BIT -> "s16"
                C.ENCODING_PCM_24BIT -> "s24"
                C.ENCODING_PCM_32BIT -> "s32"
                C.ENCODING_PCM_FLOAT -> "float"
                else -> return
            }
            send("format") {
                put("format", format)
                put("samplerate", audioTrackConfig.sampleRate)
                put("channelCount", Integer.bitCount(audioTrackConfig.channelConfig))
            }
        }
    }

    // Les URLs de segments sont signées : jamais journalisées ni remontées avec leur signature.
    private fun redact(text: String): String = text.replace(Regex("""\?[^\s"'<>]*"""), "?<signature masquée>")
}
