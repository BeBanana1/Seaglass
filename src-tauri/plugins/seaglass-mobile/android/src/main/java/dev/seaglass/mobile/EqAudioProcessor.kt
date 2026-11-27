package dev.seaglass.mobile

import android.util.Log
import androidx.annotation.OptIn
import androidx.media3.common.C
import androidx.media3.common.audio.AudioProcessor.AudioFormat
import androidx.media3.common.audio.BaseAudioProcessor
import androidx.media3.common.util.UnstableApi
import java.nio.ByteBuffer
import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.pow
import kotlin.math.sin
import kotlin.math.sqrt

/** Filter of an equalizer profile (#467), as Rust sends it (`player::eq::Filter`). */
internal data class EqFilterSpec(val kind: String, val freq: Double, val gain: Double, val q: Double)

/** Applied profile: effective preamp (dB) then filters. */
internal data class EqSpec(val preamp: Double, val filters: List<EqFilterSpec>)

/**
 * Android's parametric equalizer (#467): preamp then biquads from R. Bristow-Johnson's
 * "Audio EQ Cookbook", the same as `libavfilter` on the desktop (`equalizer`, `bass`,
 * `treble` with `t=q`) and as AutoEQ.
 *
 * Placed in the `DefaultAudioSink` (`Playback.build`): it receives 16-bit PCM (the sink
 * brings 24-bit down to 16 without float output) or float. Always active for these two
 * formats: without a profile, it copies the signal as is, which allows enabling the
 * equalizer on the fly without reconfiguring the sink. The profile is set from the main
 * thread (`profile`) and taken into account at the next buffer, on the audio thread.
 */
@OptIn(UnstableApi::class)
internal class EqAudioProcessor : BaseAudioProcessor() {
    @Volatile
    var profile: EqSpec? = null

    private var applied: EqSpec? = null
    private var preampLinear = 1.0
    private var stages: Array<Biquad> = emptyArray()

    override fun onConfigure(inputAudioFormat: AudioFormat): AudioFormat {
        if (inputAudioFormat.encoding != C.ENCODING_PCM_16BIT &&
            inputAudioFormat.encoding != C.ENCODING_PCM_FLOAT
        ) {
            Log.w(TAG, "format ${inputAudioFormat.encoding} not supported: equalizer inactive")
            return AudioFormat.NOT_SET
        }
        return inputAudioFormat
    }

    override fun onFlush() {
        // New configuration or seek: coefficients recomputed for the current sample rate,
        // filter memory reset to zero.
        applied = null
        stages = emptyArray()
    }

    override fun onReset() {
        onFlush()
    }

    override fun queueInput(inputBuffer: ByteBuffer) {
        val size = inputBuffer.remaining()
        if (size == 0) return
        val spec = profile
        if (spec != applied) rebuild(spec)
        val out = replaceOutputBuffer(size)
        if (spec == null) {
            out.put(inputBuffer)
            out.flip()
            return
        }
        val channels = inputAudioFormat.channelCount
        if (inputAudioFormat.encoding == C.ENCODING_PCM_16BIT) {
            var c = 0
            while (inputBuffer.remaining() >= 2) {
                val s = process(c, inputBuffer.getShort() / 32768.0) * 32768.0
                out.putShort(s.coerceIn(-32768.0, 32767.0).toInt().toShort())
                c = if (c + 1 == channels) 0 else c + 1
            }
        } else {
            var c = 0
            while (inputBuffer.remaining() >= 4) {
                val s = process(c, inputBuffer.getFloat().toDouble())
                out.putFloat(s.coerceIn(-1.0, 1.0).toFloat())
                c = if (c + 1 == channels) 0 else c + 1
            }
        }
        out.flip()
    }

    private fun process(channel: Int, sample: Double): Double {
        var s = sample * preampLinear
        for (stage in stages) s = stage.process(channel, s)
        return s
    }

    private fun rebuild(spec: EqSpec?) {
        applied = spec
        if (spec == null) {
            stages = emptyArray()
            preampLinear = 1.0
            return
        }
        val rate = inputAudioFormat.sampleRate.toDouble()
        val channels = inputAudioFormat.channelCount
        preampLinear = 10.0.pow(spec.preamp / 20.0)
        // A 0 dB filter does nothing; beyond Nyquist, it makes no sense.
        stages = spec.filters
            .filter { it.gain != 0.0 && it.freq < rate * 0.49 }
            .map { Biquad(it, rate, channels) }
            .toTypedArray()
        Log.i(TAG, "equalizer: ${stages.size} filters at ${rate.toInt()} Hz, preamp ${spec.preamp} dB")
    }

    /** Biquad in transposed direct form II, one memory per channel. */
    private class Biquad(f: EqFilterSpec, rate: Double, channels: Int) {
        private val b0: Double
        private val b1: Double
        private val b2: Double
        private val a1: Double
        private val a2: Double
        private val z1 = DoubleArray(channels)
        private val z2 = DoubleArray(channels)

        init {
            val a = 10.0.pow(f.gain / 40.0)
            val w0 = 2 * PI * f.freq / rate
            val cw = cos(w0)
            val alpha = sin(w0) / (2 * f.q)
            val s = 2 * sqrt(a) * alpha
            val c = when (f.kind) {
                "lowShelf" -> doubleArrayOf(
                    a * ((a + 1) - (a - 1) * cw + s),
                    2 * a * ((a - 1) - (a + 1) * cw),
                    a * ((a + 1) - (a - 1) * cw - s),
                    (a + 1) + (a - 1) * cw + s,
                    -2 * ((a - 1) + (a + 1) * cw),
                    (a + 1) + (a - 1) * cw - s,
                )
                "highShelf" -> doubleArrayOf(
                    a * ((a + 1) + (a - 1) * cw + s),
                    -2 * a * ((a - 1) + (a + 1) * cw),
                    a * ((a + 1) + (a - 1) * cw - s),
                    (a + 1) - (a - 1) * cw + s,
                    2 * ((a - 1) - (a + 1) * cw),
                    (a + 1) - (a - 1) * cw - s,
                )
                else -> doubleArrayOf(
                    1 + alpha * a,
                    -2 * cw,
                    1 - alpha * a,
                    1 + alpha / a,
                    -2 * cw,
                    1 - alpha / a,
                )
            }
            b0 = c[0] / c[3]
            b1 = c[1] / c[3]
            b2 = c[2] / c[3]
            a1 = c[4] / c[3]
            a2 = c[5] / c[3]
        }

        fun process(ch: Int, x: Double): Double {
            val y = b0 * x + z1[ch]
            z1[ch] = b1 * x - a1 * y + z2[ch]
            z2[ch] = b2 * x - a2 * y
            return y
        }
    }

    private companion object {
        const val TAG = "SeaglassPlayback"
    }
}
