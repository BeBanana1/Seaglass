package dev.seaglass.mobile

import android.content.Context
import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.TrackSelectionParameters.AudioOffloadPreferences
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.exoplayer.audio.AudioOffloadSupport
import androidx.media3.exoplayer.audio.DefaultAudioOffloadSupportProvider
import androidx.media3.exoplayer.audio.DefaultAudioSink

/**
 * Audio offload (#474) of `Playback`'s player: which formats the audio chip receives, and
 * whether offload is allowed for the current track. Moved out of `Playback.kt` (#45).
 */
@OptIn(UnstableApi::class)
internal class AudioOffload {
    /** Last offload choice set on the player (`null`: nothing set yet). */
    private var allowed: Boolean? = null

    /** New player: offload disabled by default in Media3, set again at the next `play`. */
    fun reset() {
        allowed = null
    }

    /**
     * Audio offload (#474): the FLAC goes compressed to the audio chip, which decodes it, and
     * the CPU sleeps between two buffer refills. Media3 only uses it if the device can decode
     * this format **and** chain two tracks without a gap (`isGaplessSupportRequired`);
     * otherwise, decoding in the app as before. Off while an equalizer profile is active:
     * with offload, the sink's `AudioProcessor`s are bypassed and the equalizer would be
     * ignored. The volume (normalization included) is still applied by the `AudioTrack`. The
     * parameters only change if the choice changes: a change reselects the audio track (the
     * sink reconfigures, a short dropout is possible when the equalizer is enabled).
     */
    fun update(p: ExoPlayer, eq: EqSpec?) {
        val next = eq == null
        if (next == allowed) return
        allowed = next
        val mode = if (next) {
            AudioOffloadPreferences.AUDIO_OFFLOAD_MODE_ENABLED
        } else {
            AudioOffloadPreferences.AUDIO_OFFLOAD_MODE_DISABLED
        }
        val preferences = AudioOffloadPreferences.Builder()
            .setAudioOffloadMode(mode)
            .setIsGaplessSupportRequired(true)
            .build()
        p.trackSelectionParameters = p.trackSelectionParameters.buildUpon()
            .setAudioOffloadPreferences(preferences)
            .build()
        Log.i(TAG, "offload ${if (next) "allowed" else "off (equalizer active)"}")
    }

    /**
     * Formats the audio chip receives in offload (#474), HE-AAC excluded (#492): on the S21,
     * HE-AAC offload to Bluetooth headphones does open the output, but the HAL never renders
     * anything (`get_render_position: No data available` in a loop) and ExoPlayer recreates
     * the AudioTrack every ~5 s, with no sound at all. AAC-LC (High quality) works there,
     * checked on headphones and on the speaker. Decided for each track.
     */
    fun support(context: Context): DefaultAudioSink.AudioOffloadSupportProvider {
        val system = DefaultAudioOffloadSupportProvider(context)
        return DefaultAudioSink.AudioOffloadSupportProvider { format, attributes ->
            if (isHeAac(format.codecs)) {
                Log.i(TAG, "offload refused for HE-AAC (${format.codecs})")
                AudioOffloadSupport.DEFAULT_UNSUPPORTED
            } else {
                system.getAudioOffloadSupport(format, attributes)
            }
        }
    }

    /** HE-AAC (v1: `mp4a.40.5`, v2: `mp4a.40.29`), TIDAL's Low quality. */
    private fun isHeAac(codecs: String?): Boolean =
        codecs?.split(',')?.any { it.trim() in HE_AAC_CODECS } == true

    private companion object {
        /** Same log tag as `Playback`: the logcat filter in `docs/android.md` keeps working. */
        const val TAG = "SeaglassPlayback"
        val HE_AAC_CODECS = setOf("mp4a.40.5", "mp4a.40.29")
    }
}
