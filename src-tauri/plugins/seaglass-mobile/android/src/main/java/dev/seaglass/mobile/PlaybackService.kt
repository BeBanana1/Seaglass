package dev.seaglass.mobile

import android.app.PendingIntent
import android.content.Intent
import android.os.Bundle
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import androidx.annotation.OptIn
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.CommandButton
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService
import androidx.media3.session.SessionCommand
import androidx.media3.session.SessionResult
import com.google.common.util.concurrent.Futures
import com.google.common.util.concurrent.ListenableFuture

/**
 * Playback service (#24): keeps the app alive with the screen off and publishes the media
 * session (notification, lock screen, Bluetooth keys). The player belongs to `Playback`.
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
    companion object {
        const val SHUFFLE = "dev.seaglass.SHUFFLE"
        const val REPEAT = "dev.seaglass.REPEAT"

        /**
         * How long the service stays in the foreground after a pause (#247), as Media3 does
         * since 1.6.
         */
        private const val PAUSED_HOLD_MS = 10 * 60_000L

        /** Running service, to refresh the buttons when Rust changes their state. */
        var current: PlaybackService? = null
            private set
    }

    private var session: MediaSession? = null

    /** The session's image loader: the widget reuses the artwork it already loaded (#329). */
    internal val bitmapLoader get() = session?.bitmapLoader

    private val main = Handler(Looper.getMainLooper())

    /** End of the foreground hold (`SystemClock.elapsedRealtime`), 0: never played. */
    private var holdUntil = 0L

    /** Hold elapsed: the notification goes back to a plain notification. */
    private val endHold = Runnable {
        session?.let { onUpdateNotification(it, false) }
    }

    /**
     * Every notification update goes through here, with what Media3 decided: foreground
     * during playback only. On pause, it therefore handed the service back to the
     * background; with the app not visible, Android stopped it a minute later ("app
     * idle"), the media session was destroyed and the next press on the headphones
     * reached nowhere (#247). So the service stays in the foreground for
     * `PAUSED_HOLD_MS` after the last playback, as long as a track is loaded.
     */
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        val now = SystemClock.elapsedRealtime()
        main.removeCallbacks(endHold)
        if (startInForegroundRequired) holdUntil = now + PAUSED_HOLD_MS
        val hold = !startInForegroundRequired && now < holdUntil && resumable(session.player)
        if (hold) main.postDelayed(endHold, holdUntil - now)
        super.onUpdateNotification(session, startInForegroundRequired || hold)
    }

    /** A track is loaded and ready: same states in which Media3 keeps the foreground. */
    private fun resumable(player: Player): Boolean = player.mediaItemCount > 0 &&
        (player.playbackState == Player.STATE_READY || player.playbackState == Player.STATE_BUFFERING)

    override fun onCreate() {
        super.onCreate()
        // Restarted by the system without a player (process recreated): nothing to publish.
        val player = Playback.remote ?: return stopSelf()
        val created = MediaSession.Builder(this, player)
            .setSessionActivity(nowPlayingIntent())
            .setCallback(callback)
            .setCustomLayout(layout())
            .build()
        session = created
        current = this
        // No controller connects at startup: the session is added here so that the
        // notification shows up as soon as playback starts.
        addSession(created)
    }

    /** Shuffle or repeat changed (from the app or the notification). */
    fun refreshLayout() {
        session?.setCustomLayout(layout())
    }

    /**
     * Tap on the notification (#163): `NowPlayingActivity` relays the request to Rust then
     * brings the app to the foreground, which opens on "Now playing".
     */
    private fun nowPlayingIntent(): PendingIntent = PendingIntent.getActivity(
        this,
        0,
        Intent(this, NowPlayingActivity::class.java),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /** Shuffle and repeat buttons (#163), state and labels coming from Rust. */
    private fun layout(): List<CommandButton> {
        val modes = Playback.modes
        val shuffle = CommandButton.Builder(
            if (modes.shuffle) CommandButton.ICON_SHUFFLE_ON else CommandButton.ICON_SHUFFLE_OFF,
        )
            .setDisplayName(modes.shuffleLabel)
            .setSessionCommand(SessionCommand(SHUFFLE, Bundle.EMPTY))
            .build()
        val repeat = CommandButton.Builder(
            when (modes.repeat) {
                "one" -> CommandButton.ICON_REPEAT_ONE
                "all" -> CommandButton.ICON_REPEAT_ALL
                else -> CommandButton.ICON_REPEAT_OFF
            },
        )
            .setDisplayName(modes.repeatLabel)
            .setSessionCommand(SessionCommand(REPEAT, Bundle.EMPTY))
            .build()
        return listOf(shuffle, repeat)
    }

    private val callback = object : MediaSession.Callback {
        override fun onConnect(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
        ): MediaSession.ConnectionResult {
            val commands = MediaSession.ConnectionResult.DEFAULT_SESSION_COMMANDS.buildUpon()
                .add(SessionCommand(SHUFFLE, Bundle.EMPTY))
                .add(SessionCommand(REPEAT, Bundle.EMPTY))
                .build()
            // No controller is filtered (#145, Buds and lock screen): only the commands that
            // none of them needs to use are removed (#254).
            val player = MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                .removeAll(*RemotePlayer.BLOCKED)
                .build()
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(commands)
                .setAvailablePlayerCommands(player)
                .build()
        }

        // Like the other buttons: Rust decides the next state and sends it back.
        override fun onCustomCommand(
            session: MediaSession,
            controller: MediaSession.ControllerInfo,
            customCommand: SessionCommand,
            args: Bundle,
        ): ListenableFuture<SessionResult> {
            val result = when (customCommand.customAction) {
                SHUFFLE -> {
                    Playback.command("shuffle")
                    SessionResult.RESULT_SUCCESS
                }
                REPEAT -> {
                    Playback.command("repeat")
                    SessionResult.RESULT_SUCCESS
                }
                else -> SessionResult.RESULT_ERROR_NOT_SUPPORTED
            }
            return Futures.immediateFuture(SessionResult(result))
        }
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** App swiped away from recents: the service is only kept if music is playing. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        main.removeCallbacks(endHold)
        if (current === this) current = null
        session?.release()
        session = null
        super.onDestroy()
    }
}

/**
 * The player as the session sees it: play, pause, seek, next, previous, repeat and shuffle
 * go back to Rust, which holds the queue (like SMTC on the desktop). Nothing is applied
 * directly to the ExoPlayer, and what the session has no business doing (changing the
 * entries, the speed, the volume) is not offered to controllers (#254).
 */
@OptIn(UnstableApi::class)
class RemotePlayer(player: ExoPlayer) : ForwardingPlayer(player) {
    companion object {
        /**
         * Commands never offered: ExoPlayer's window (current track + next one) and its
         * volume belong to the engine, driven by Rust. Also removed from the commands
         * granted to controllers in `onConnect`.
         */
        val BLOCKED = intArrayOf(
            Player.COMMAND_SET_MEDIA_ITEM,
            Player.COMMAND_CHANGE_MEDIA_ITEMS,
            Player.COMMAND_SET_SPEED_AND_PITCH,
            Player.COMMAND_SET_VOLUME,
        )
    }

    /** Session listeners: notified when Rust changes the repeat or the shuffle. */
    private val listeners = mutableListOf<Player.Listener>()

    override fun addListener(listener: Player.Listener) {
        listeners.add(listener)
        super.addListener(listener)
    }

    override fun removeListener(listener: Player.Listener) {
        listeners.remove(listener)
        super.removeListener(listener)
    }

    override fun play() = Playback.command("play")

    override fun pause() = Playback.command("pause")

    override fun setPlayWhenReady(playWhenReady: Boolean) = Playback.command(if (playWhenReady) "play" else "pause")

    // The session prepares an idle player before `play`: Rust is the one reloading the track.
    override fun prepare() {}

    override fun stop() = Playback.command("pause")

    override fun seekToNext() = Playback.command("next")

    override fun seekToNextMediaItem() = Playback.command("next")

    override fun seekToPrevious() = Playback.command("previous")

    override fun seekToPreviousMediaItem() = Playback.command("previous")

    override fun seekTo(positionMs: Long) = Playback.command("seek", positionMs)

    override fun seekTo(mediaItemIndex: Int, positionMs: Long) {
        if (mediaItemIndex == currentMediaItemIndex) Playback.command("seek", positionMs)
    }

    // Repeat and shuffle of the queue, not of ExoPlayer's window: its repeat is only used
    // for "repeat the track" (`loop`) and ignores "repeat the queue".
    override fun setRepeatMode(repeatMode: Int) = Playback.setRepeat(
        when (repeatMode) {
            Player.REPEAT_MODE_ONE -> "one"
            Player.REPEAT_MODE_ALL -> "all"
            else -> "off"
        },
    )

    override fun getRepeatMode(): Int = when (Playback.modes.repeat) {
        "one" -> Player.REPEAT_MODE_ONE
        "all" -> Player.REPEAT_MODE_ALL
        else -> Player.REPEAT_MODE_OFF
    }

    override fun setShuffleModeEnabled(shuffleModeEnabled: Boolean) = Playback.setShuffle(shuffleModeEnabled)

    override fun getShuffleModeEnabled(): Boolean = Playback.modes.shuffle

    /**
     * State received from Rust (`sessionModes`, main thread). The session does not reread the
     * getters: it takes the value from the events. ExoPlayer only emits them for its own
     * repeat ("repeat the track"), before Rust sends the state back: that one comes after.
     */
    fun modesChanged() {
        val repeat = repeatMode
        val shuffle = shuffleModeEnabled
        for (listener in listeners.toList()) {
            listener.onRepeatModeChanged(repeat)
            listener.onShuffleModeEnabledChanged(shuffle)
        }
    }

    // The window often has a single entry: next and previous are always offered.
    override fun getAvailableCommands(): Player.Commands = super.getAvailableCommands()
        .buildUpon()
        .addAll(
            Player.COMMAND_PLAY_PAUSE,
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
        )
        .removeAll(*BLOCKED)
        .build()

    override fun isCommandAvailable(command: Int): Boolean = availableCommands.contains(command)
}
