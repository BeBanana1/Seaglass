package ch.bebanana.seaglass.spike

import android.app.Activity
import android.content.Intent
import android.media.AudioDeviceInfo
import android.media.AudioManager
import android.net.Uri
import android.os.Bundle
import android.util.Log
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.Format
import androidx.media3.common.MediaItem
import androidx.media3.common.MimeTypes
import androidx.media3.common.PlaybackException
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.DecoderReuseEvaluation
import androidx.media3.exoplayer.DefaultRenderersFactory
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.analytics.AnalyticsListener
import androidx.media3.exoplayer.audio.AudioSink
import androidx.media3.ui.PlayerControlView

/**
 * Spike #24 : vérifier que Media3 lit les MPD de TIDAL (FLAC en fMP4, segments signés)
 * et noter le format décodé. Écran unique, jetable.
 *
 * Lancement depuis le PC (voir `spike/android/README.md`) :
 *   adb shell am start -a android.intent.action.VIEW -n ch.bebanana.seaglass.spike/.MainActivity -d <url du MPD>
 * Extras : `--ez append true` (ajoute à la file au lieu de remplacer, pour la transition),
 * `--ez float true` (sortie PCM flottante, pour garder les 24 bits). Sans URL,
 * `--el seek <ms>` saute dans le titre en cours.
 */
@OptIn(UnstableApi::class)
class MainActivity : Activity() {
    private var player: ExoPlayer? = null
    private var floatOutput = false
    private var seekStarted: Long? = null
    private lateinit var log: TextView
    private lateinit var controls: PlayerControlView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        log = TextView(this).apply {
            setTextIsSelectable(true)
            typeface = android.graphics.Typeface.MONOSPACE
            textSize = 11f
            setPadding(24, 24, 24, 24)
        }
        controls = PlayerControlView(this).apply { showTimeoutMs = 0 }
        setContentView(
            LinearLayout(this).apply {
                orientation = LinearLayout.VERTICAL
                fitsSystemWindows = true
                addView(controls)
                addView(ScrollView(this@MainActivity).apply { addView(log) })
            },
        )
        handle(intent)
    }

    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        handle(intent)
    }

    override fun onDestroy() {
        player?.release()
        player = null
        super.onDestroy()
    }

    private fun handle(intent: Intent) {
        val seek = intent.getLongExtra("seek", -1)
        if (seek >= 0 && intent.data == null) {
            seekStarted = System.nanoTime()
            player?.seekTo(seek)
            return say("Saut à $seek ms")
        }
        val uri = intent.data ?: return say("En attente d'une URL (adb shell am start … -d <mpd>)")
        val wantFloat = intent.getBooleanExtra("float", false)
        if (player == null || wantFloat != floatOutput) {
            player?.release()
            floatOutput = wantFloat
            player = buildPlayer()
            say("Lecteur créé (sortie flottante : $floatOutput)")
        }
        val p = player ?: return
        val item = MediaItem.Builder().setUri(uri).setMimeType(MimeTypes.APPLICATION_MPD).build()
        if (intent.getBooleanExtra("append", false)) {
            p.addMediaItem(item)
            say("Ajouté à la file : ${redact(uri)} (${p.mediaItemCount} entrées)")
        } else {
            p.setMediaItem(item)
            p.prepare()
            p.play()
            say("Lecture : ${redact(uri)}")
        }
        logOutputs()
    }

    private fun buildPlayer(): ExoPlayer {
        val renderers = DefaultRenderersFactory(this).setEnableAudioFloatOutput(floatOutput)
        return ExoPlayer.Builder(this, renderers).build().also { p ->
            controls.player = p
            p.addAnalyticsListener(analytics)
            p.addListener(
                object : Player.Listener {
                    override fun onMediaItemTransition(mediaItem: MediaItem?, reason: Int) {
                        say("Transition (raison $reason) → ${mediaItem?.localConfiguration?.uri?.let { redact(it) }}")
                    }

                    override fun onPlaybackStateChanged(state: Int) {
                        val name = when (state) {
                            Player.STATE_IDLE -> "IDLE"
                            Player.STATE_BUFFERING -> "BUFFERING"
                            Player.STATE_READY -> "READY"
                            Player.STATE_ENDED -> "ENDED"
                            else -> "$state"
                        }
                        val sinceSeek = seekStarted?.takeIf { state == Player.STATE_READY }
                            ?.let { " (${(System.nanoTime() - it) / 1_000_000} ms après le saut)" }
                        if (sinceSeek != null) seekStarted = null
                        say("État : $name${sinceSeek.orEmpty()}")
                    }
                },
            )
        }
    }

    private val analytics = object : AnalyticsListener {
        override fun onAudioInputFormatChanged(
            eventTime: AnalyticsListener.EventTime,
            format: Format,
            decoderReuseEvaluation: DecoderReuseEvaluation?,
        ) {
            say(
                "Entrée : ${format.sampleMimeType} codecs=${format.codecs} " +
                    "${format.sampleRate} Hz ${format.channelCount} ch " +
                    "pcm=${encoding(format.pcmEncoding)} débit=${kbps(format.averageBitrate)}/${kbps(format.peakBitrate)} kbps",
            )
        }

        override fun onAudioDecoderInitialized(
            eventTime: AnalyticsListener.EventTime,
            decoderName: String,
            initializedTimestampMs: Long,
            initializationDurationMs: Long,
        ) {
            say("Décodeur : $decoderName (${initializationDurationMs} ms)")
        }

        override fun onAudioTrackInitialized(
            eventTime: AnalyticsListener.EventTime,
            audioTrackConfig: AudioSink.AudioTrackConfig,
        ) {
            say(
                "AudioTrack : ${encoding(audioTrackConfig.encoding)} ${audioTrackConfig.sampleRate} Hz " +
                    "offload=${audioTrackConfig.offload} tampon=${audioTrackConfig.bufferSize} o",
            )
        }

        override fun onAudioUnderrun(
            eventTime: AnalyticsListener.EventTime,
            bufferSize: Int,
            bufferSizeMs: Long,
            elapsedSinceLastFeedMs: Long,
        ) {
            say("Sous-alimentation audio (tampon $bufferSizeMs ms)")
        }

        override fun onPlayerError(eventTime: AnalyticsListener.EventTime, error: PlaybackException) {
            say("ERREUR ${error.errorCodeName} : ${redact(error.cause?.toString() ?: error.message.orEmpty())}")
        }
    }

    private fun logOutputs() {
        val audio = getSystemService(AudioManager::class.java)
        val outputs = audio.getDevices(AudioManager.GET_DEVICES_OUTPUTS).joinToString { d ->
            val type = when (d.type) {
                AudioDeviceInfo.TYPE_BLUETOOTH_A2DP -> "A2DP"
                AudioDeviceInfo.TYPE_BLE_HEADSET -> "BLE"
                AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> "haut-parleur"
                AudioDeviceInfo.TYPE_USB_HEADSET, AudioDeviceInfo.TYPE_USB_DEVICE -> "USB"
                AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_WIRED_HEADSET -> "filaire"
                else -> "type ${d.type}"
            }
            "${d.productName} ($type)"
        }
        say("Sorties : $outputs")
    }

    private fun encoding(value: Int): String = when (value) {
        C.ENCODING_PCM_16BIT -> "s16"
        C.ENCODING_PCM_24BIT -> "s24"
        C.ENCODING_PCM_32BIT -> "s32"
        C.ENCODING_PCM_FLOAT -> "float"
        Format.NO_VALUE -> "-"
        else -> "encodage $value"
    }

    private fun kbps(bitrate: Int): String = if (bitrate == Format.NO_VALUE) "?" else "${bitrate / 1000}"

    private fun say(line: String) {
        Log.i(TAG, line)
        runOnUiThread { log.append("$line\n") }
    }

    private companion object {
        const val TAG = "SeaglassSpike"

        // Les URLs de segments sont signées : jamais affichées avec leur signature.
        fun redact(text: String): String = text.replace(Regex("""\?[^\s"'<>]*"""), "?<signature masquée>")

        fun redact(uri: Uri): String = redact(uri.toString())
    }
}
