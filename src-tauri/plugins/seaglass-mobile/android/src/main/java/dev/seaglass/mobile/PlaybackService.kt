package dev.seaglass.mobile

import android.app.PendingIntent
import android.content.Intent
import android.os.Bundle
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
 * Service de lecture (#24) : garde l'app en vie écran éteint et publie la session média
 * (notification, écran verrouillé, touches Bluetooth). Le lecteur appartient à `Playback`.
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
    companion object {
        const val SHUFFLE = "dev.seaglass.SHUFFLE"
        const val REPEAT = "dev.seaglass.REPEAT"

        /** Service en cours, pour remettre à jour les boutons quand Rust change leur état. */
        var current: PlaybackService? = null
            private set
    }

    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        // Relancé par le système sans lecteur (processus recréé) : rien à publier.
        val player = Playback.remote ?: return stopSelf()
        val created = MediaSession.Builder(this, player)
            .setSessionActivity(nowPlayingIntent())
            .setCallback(callback)
            .setCustomLayout(layout())
            .build()
        session = created
        current = this
        // Aucun contrôleur ne se connecte à l'ouverture : la session est ajoutée ici pour
        // que la notification apparaisse dès la lecture.
        addSession(created)
    }

    /** Aléatoire ou répétition changés (depuis l'app ou la notification). */
    fun refreshLayout() {
        session?.setCustomLayout(layout())
    }

    /**
     * Appui sur la notification (#163) : `NowPlayingActivity` relaie la demande à Rust puis
     * ramène l'app au premier plan, qui s'ouvre sur « En lecture ».
     */
    private fun nowPlayingIntent(): PendingIntent = PendingIntent.getActivity(
        this,
        0,
        Intent(this, NowPlayingActivity::class.java),
        PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
    )

    /** Boutons aléatoire et répétition (#163), état et libellés venus de Rust. */
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
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(commands)
                .build()
        }

        // Comme les autres boutons : Rust décide de l'état suivant et le renvoie.
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

    /** App balayée des récents : on ne garde le service que si la musique joue. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
        if (current === this) current = null
        session?.release()
        session = null
        super.onDestroy()
    }
}

/**
 * Le lecteur tel que la session le voit : lecture, pause, saut, suivant et précédent
 * repartent vers Rust, qui tient la file (comme SMTC sur le bureau). Rien n'est appliqué
 * directement à l'ExoPlayer.
 */
@OptIn(UnstableApi::class)
class RemotePlayer(player: ExoPlayer) : ForwardingPlayer(player) {
    override fun play() = Playback.command("play")

    override fun pause() = Playback.command("pause")

    override fun setPlayWhenReady(playWhenReady: Boolean) = Playback.command(if (playWhenReady) "play" else "pause")

    // La session prépare un lecteur inactif avant `play` : c'est Rust qui recharge le titre.
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

    // La fenêtre n'a souvent qu'une entrée : suivant et précédent restent toujours proposés.
    override fun getAvailableCommands(): Player.Commands = super.getAvailableCommands()
        .buildUpon()
        .addAll(
            Player.COMMAND_PLAY_PAUSE,
            Player.COMMAND_SEEK_TO_NEXT,
            Player.COMMAND_SEEK_TO_NEXT_MEDIA_ITEM,
            Player.COMMAND_SEEK_TO_PREVIOUS,
            Player.COMMAND_SEEK_TO_PREVIOUS_MEDIA_ITEM,
        )
        .build()

    override fun isCommandAvailable(command: Int): Boolean = availableCommands.contains(command)
}
