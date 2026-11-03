package dev.seaglass.mobile

import android.content.Intent
import androidx.annotation.OptIn
import androidx.media3.common.ForwardingPlayer
import androidx.media3.common.Player
import androidx.media3.common.util.UnstableApi
import androidx.media3.exoplayer.ExoPlayer
import androidx.media3.session.MediaSession
import androidx.media3.session.MediaSessionService

/**
 * Service de lecture (#24) : garde l'app en vie écran éteint et publie la session média
 * (notification, écran verrouillé, touches Bluetooth). Le lecteur appartient à `Playback`.
 */
class PlaybackService : MediaSessionService() {
    private var session: MediaSession? = null

    override fun onCreate() {
        super.onCreate()
        // Relancé par le système sans lecteur (processus recréé) : rien à publier.
        val player = Playback.remote ?: return stopSelf()
        val created = MediaSession.Builder(this, player).build()
        session = created
        // Aucun contrôleur ne se connecte à l'ouverture : la session est ajoutée ici pour
        // que la notification apparaisse dès la lecture.
        addSession(created)
    }

    override fun onGetSession(controllerInfo: MediaSession.ControllerInfo): MediaSession? = session

    /** App balayée des récents : on ne garde le service que si la musique joue. */
    override fun onTaskRemoved(rootIntent: Intent?) {
        val player = session?.player
        if (player == null || !player.playWhenReady || player.mediaItemCount == 0) stopSelf()
    }

    override fun onDestroy() {
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
