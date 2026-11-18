# Android spike (#24)

Throwaway single-screen app: check that **Media3 (ExoPlayer)** plays TIDAL's MPDs (FLAC in fMP4, `SegmentTemplate` + `SegmentTimeline`, signed segments) and record the format actually decoded. It does not talk to TIDAL: the PC serves it the MPD, as it did for mpv in the phase 0 spike.

Nothing is written to disk (principle 1): the MPD stays in memory on the PC, Media3 streams the segments.

## Requirements

- Android Studio (Android SDK 35; the NDK is not needed for this spike).
- Phone in **USB debugging** (Developer options), connected over USB, listed by `adb devices`.
- The Python spike signed in with PKCE: `python spike\play.py login --pkce`.

## Build and install

Open `spike/android/` in Android Studio (it generates the Gradle wrapper), then Run on the phone. From the command line, once the wrapper is generated:

```powershell
cd spike\android; .\gradlew.bat installDebug
```

## Play a track

```powershell
python spike\play.py serve <id> --quality lossless   # or hires, high, low
adb reverse tcp:8765 tcp:8765
adb shell am start -a android.intent.action.VIEW -n ch.bebanana.seaglass.spike/.MainActivity -d http://127.0.0.1:8765/manifest.mpd
```

`serve` prints both `adb` commands. `adb reverse` makes the PC's `127.0.0.1:8765` reachable at the same place on the phone: nothing is opened on the local network.

`am start` extras:
- `--ez float true`: float PCM output (otherwise Android decodes 24-bit FLAC to 16-bit).
- `--el seek <ms>`, without `-d`: seek in the current track (time until `READY` shown).
- `--ez append true`: adds the track to the queue instead of replacing it (transition test). The second track needs its own port: `serve <id2> --port 8766`, `adb reverse tcp:8766 tcp:8766`, URL on `:8766`.

## Reading the results

The screen and `adb logcat -s SeaglassSpike` show: input format (`audio/flac`, sample rate, bitrate), decoder (`c2.android.flac.decoder`…), `AudioTrack` configuration (encoding, sample rate, offload), audio outputs, errors. URL signatures are masked.

## To check

| Case | Expected |
|---|---|
| `lossless` (FLAC 16/44.1) | plays, `audio/flac`, AudioTrack `s16` 44100 |
| `hires` (24/96 or more) | plays; `float` output with `--ez float true` |
| `high` (AAC-LC 320) and `low` (HE-AAC 96) | plays |
| Seeking in the bar | quick resume, no error |
| `append` of a second track | transition with no audible gap |
| Screen off | without a playback service, playback may stop: expected for the spike |

## Results (2026-11-03, Samsung S21 SM-G991B, Android 15, Media3 1.5.1, speaker)

| Requested | Decoder | Input | AudioTrack |
|---|---|---|---|
| `lossless` (Billie Jean, 1632061) | `c2.sec.flac.decoder` (Samsung, hardware) | `audio/flac` 44100 Hz, 1,026 kbps | `s16` 44100 |
| `hires` + `float` (97320484, 24/96) | `c2.sec.flac.decoder` | `audio/flac`, sample rate **0** in the `Format` (declared nowhere before decoding), 3,079 kbps | `float` 96000 |
| `high` (AAC-LC 320) | `c2.android.aac.decoder` | `mp4a.40.2` 320 kbps | `s16` 44100 |
| `low` (HE-AAC 96) | `c2.android.aac.decoder` | `mp4a.40.5` 96 kbps | `s16` 44100 |

- **TIDAL's MPD is played as is** by Media3's DASH module: no extension (`libflac`), no transcoding, no manifest adaptation. Nothing justifies libmpv.
- Seeking: `READY` 240–400 ms after `seekTo` (24/96 and 16/44.1).
- Queue: FLAC → HE-AAC switch at the end of a track, automatic transition (reason `AUTO`), no error. The audible gap has not been assessed (listening test to do).
- An MPD served two minutes earlier still plays: signatures last at least that long.
- For the app: the real format is read in `onAudioTrackInitialized` (or `playbackinfopostpaywall`), not in the input `Format`.
