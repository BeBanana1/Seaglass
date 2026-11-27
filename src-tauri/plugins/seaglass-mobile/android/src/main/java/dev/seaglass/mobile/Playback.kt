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
import androidx.media3.exoplayer.DefaultLoadControl
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.exoplayer.audio.DefaultAudioSink
import app.tauri.plugin.Channel
import app.tauri.plugin.JSObject
import kotlin.math.min
import kotlin.math.pow

/**
 * Android playback engine (#24): the app's ExoPlayer and its window of at most 2 entries
 * (current track + preloaded next one), like mpv on the desktop. Rust decides everything
 * (`engine::Op`); what happens here goes back up as events through `events`.
 *
 * Everything runs on the main thread, ExoPlayer's. The object lives as long as the process:
 * the service and the session can be recreated around it.
 */
@OptIn(UnstableApi::class)
internal object Playback {
    private const val TAG = "SeaglassPlayback"
    /**
     * Position sent during playback (#147): the UI interpolates between two sends and Rust
     * only needs it for preloading (30 s before the end) and for saving. Spaced further apart
     * when no activity is visible (screen off, another app in the foreground). A seek, a
     * pause or a track change always sends it right away. In the foreground, every 2 s since
     * #473 (1 s before): every send wakes up everything in the UI that reads the player
     * state, which interpolates anyway.
     */
    private const val FOREGROUND_POSITION_MS = 2_000L
    private const val BACKGROUND_POSITION_MS = 5_000L

    /**
     * Burst buffering (#323). By default Media3 keeps 50 s ahead (minimum = maximum) and so
     * downloads again drip by drip throughout playback: the modem never sleeps. Here the
     * player loads up to 5 minutes ahead in one go, then asks for nothing more until it has
     * dropped back to 1 minute. A current track fits in one burst.
     * The byte limit (13 MB by default, about 2 min of 16/44.1 FLAC) is raised so as not to
     * cut the burst short. In-memory buffer only, nothing on disk.
     */
    private const val MIN_BUFFER_MS = 60_000
    private const val MAX_BUFFER_MS = 300_000
    private const val TARGET_BUFFER_BYTES = 64 * 1024 * 1024

    private val main = Handler(Looper.getMainLooper())
    private var context: Context? = null
    private var exo: ExoPlayer? = null

    /** The player as the session sees it: its commands go back to Rust. */
    var remote: RemotePlayer? = null
        private set

    private var events: Channel? = null
    private var commands: Channel? = null

    private var level = 100
    private var muted = false
    private var currentGain: Double? = null
    private var nextGain: Double? = null
    /** Equalizer (#467), in the audio sink; the profile applies to the whole playback. */
    private val equalizer = EqAudioProcessor()
    /** Audio offload (#474): what the chip decodes, and whether it is allowed now. */
    private val offload = AudioOffload()
    private var metadata: MediaMetadata = MediaMetadata.EMPTY
    private var idle: Boolean? = null
    private var buffering: Boolean? = null
    private var foreground = true
    private var observing = false

    fun onMain(block: () -> Unit) = main.post(block)

    /** Creates the player if it does not exist yet, then plugs the events into `channel`. */
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
        refreshWidget()
    }

    /** What the home screen widget shows (#329), on the main thread. */
    fun widgetState(): WidgetState {
        val live = commands != null
        val p = exo
        return WidgetState(
            live = live,
            title = metadata.title?.toString()?.takeIf { it.isNotEmpty() },
            artist = metadata.artist?.toString(),
            artwork = metadata.artworkUri,
            playing = live && p != null && p.playWhenReady && p.mediaItemCount > 0,
        )
    }

    /** Track, pause or session changed: the widget follows, if one is placed. */
    private fun refreshWidget() {
        context?.let { NowPlayingWidget.refresh(it) }
    }

    fun attachSession(channel: Channel) {
        commands = channel
        LaunchRequests.attach(channel)
        refreshWidget()
    }

    /** Action from the notification, the lock screen or the headphones. */
    fun command(name: String, positionMs: Long? = null) {
        val o = JSObject()
        o.put("command", name)
        if (positionMs != null) o.put("position", positionMs / 1000.0)
        commands?.send(o) ?: Log.w(TAG, "command \"$name\" without a plugged-in session")
    }

    /** Repeat requested by a controller (#254): `off`, `all` or `one`. */
    fun setRepeat(mode: String) {
        val o = JSObject()
        o.put("command", "setRepeat")
        o.put("repeat", mode)
        commands?.send(o) ?: Log.w(TAG, "command \"setRepeat\" without a plugged-in session")
    }

    /** Shuffle requested by a controller (#254). */
    fun setShuffle(on: Boolean) {
        val o = JSObject()
        o.put("command", "setShuffle")
        o.put("shuffle", on)
        commands?.send(o) ?: Log.w(TAG, "command \"setShuffle\" without a plugged-in session")
    }

    fun apply(args: EngineArgs) {
        val p = exo ?: throw IllegalStateException("player not started")
        when (args.op) {
            "play" -> {
                val url = args.url ?: throw IllegalArgumentException("missing url")
                p.setMediaItem(item(url, metadata), ((args.start ?: 0.0) * 1000).toLong())
                currentGain = args.gain
                nextGain = null
                equalizer.profile = args.eq
                offload.update(p, args.eq)
                applyVolume()
                p.prepare()
                p.playWhenReady = true
                startService()
            }
            "preload" -> {
                val url = args.url ?: throw IllegalArgumentException("missing url")
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
                refreshWidget()
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
                equalizer.profile = args.eq
                offload.update(p, args.eq)
                applyVolume()
            }
            else -> throw IllegalArgumentException("unknown operation: ${args.op}")
        }
    }

    /** State of the notification's shuffle and repeat buttons (#163), held by Rust. */
    var modes = SessionModes()
        private set

    fun setModes(next: SessionModes) {
        if (next == modes) return
        modes = next
        remote?.modesChanged()
        PlaybackService.current?.refreshLayout()
    }

    /** Title, artist, album and artwork of the current track, for the notification. */
    fun setNowPlaying(meta: MediaMetadata) {
        metadata = meta
        refreshWidget()
        val p = exo ?: return
        if (p.mediaItemCount == 0) return
        val index = p.currentMediaItemIndex
        val current = p.getMediaItemAt(index)
        // Same URI: Media3 updates the entry without reloading the stream.
        p.replaceMediaItem(index, current.buildUpon().setMediaMetadata(meta).build())
    }

    private fun build(appContext: Context): ExoPlayer {
        val attributes = AudioAttributes.Builder()
            .setUsage(C.USAGE_MEDIA)
            .setContentType(C.AUDIO_CONTENT_TYPE_MUSIC)
            .build()
        // Playback start unchanged: Media3's default thresholds.
        val loadControl = DefaultLoadControl.Builder()
            .setBufferDurationsMs(
                MIN_BUFFER_MS,
                MAX_BUFFER_MS,
                DefaultLoadControl.DEFAULT_BUFFER_FOR_PLAYBACK_MS,
                DefaultLoadControl.DEFAULT_BUFFER_FOR_PLAYBACK_AFTER_REBUFFER_MS,
            )
            .setTargetBufferBytes(TARGET_BUFFER_BYTES)
            .build()
        // Media3's default sink, with the equalizer at the end of the processing chain.
        val renderers = object : DefaultRenderersFactory(appContext) {
            override fun buildAudioSink(
                context: Context,
                enableFloatOutput: Boolean,
                enableAudioTrackPlaybackParams: Boolean,
            ): AudioSink = DefaultAudioSink.Builder(context)
                .setEnableFloatOutput(enableFloatOutput)
                .setEnableAudioTrackPlaybackParams(enableAudioTrackPlaybackParams)
                .setAudioProcessors(arrayOf(equalizer))
                .setAudioOffloadSupportProvider(offload.support(context))
                .build()
        }
        val player = ExoPlayer.Builder(appContext, renderers)
            .setLoadControl(loadControl)
            .setAudioAttributes(attributes, true)
            .setHandleAudioBecomingNoisy(true)
            .setWakeMode(C.WAKE_MODE_NETWORK)
            // No dynamic scheduling (`experimentalSetDynamicSchedulingEnabled`, #474): it made
            // the playback thread sleep while the AudioTrack buffer was full, but caused
            // micro-dropouts on Bluetooth headphones (#491). It can only be set when the
            // player is created, not depending on the output. In offload (AAC), the player
            // sleeps anyway (`isSleepingForOffload`): only FLAC loses out.
            .build()
        player.addListener(listener)
        player.addAnalyticsListener(analytics)
        offload.reset()
        exo = player
        remote = RemotePlayer(player)
        return player
    }

    private fun startService() {
        val ctx = context ?: return
        try {
            ctx.startService(Intent(ctx, PlaybackService::class.java))
        } catch (e: IllegalStateException) {
            // App in the background: the session will resume at the next launch.
            Log.w(TAG, "playback service not started: $e")
        }
    }

    /** A `data:` URL carries the MPD (nothing on disk or on a local port); else a direct stream. */
    private fun item(url: String, meta: MediaMetadata): MediaItem {
        val builder = MediaItem.Builder().setUri(Uri.parse(url)).setMediaMetadata(meta)
        if (url.startsWith("data:application/dash+xml")) builder.setMimeType(MimeTypes.APPLICATION_MPD)
        return builder.build()
    }

    /** App volume with mpv's cubic curve, and normalization gain (<= 0 dB). */
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
        // What Media3 already has in memory for the current track (#453), at the same rate.
        send("cached") { if (p.mediaItemCount > 0) put("value", p.bufferedPosition / 1000.0) }
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

    /** Back to the foreground: position right away, then at the foreground rate. */
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
            refreshWidget()
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
            // Only the automatic transition counts as moving to the preloaded track; replacing
            // the window (`play`) or looping (repeat One) is not one.
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
            Log.w(TAG, "playback error: $detail")
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
            Log.i(TAG, "AudioTrack: offload ${if (audioTrackConfig.offload) "active" else "inactive"}")
            // In offload (#474), the AudioTrack receives the compressed stream, decoded by the
            // audio chip.
            val format = if (audioTrackConfig.offload) "offload" else when (audioTrackConfig.encoding) {
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

    // Segment URLs are signed: never logged nor sent back up with their signature.
    private fun redact(text: String): String = text.replace(Regex("""\?[^\s"'<>]*"""), "?<signature redacted>")
}

/** Shuffle, repeat (`off`, `all`, `one`) and labels of the two buttons. */
internal data class SessionModes(
    val shuffle: Boolean = false,
    val repeat: String = "off",
    val shuffleLabel: String = "Shuffle",
    val repeatLabel: String = "Repeat",
)
