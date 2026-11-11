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
 * Service de lecture (#24) : garde l'app en vie écran éteint et publie la session média
 * (notification, écran verrouillé, touches Bluetooth). Le lecteur appartient à `Playback`.
 */
@OptIn(UnstableApi::class)
class PlaybackService : MediaSessionService() {
    companion object {
        const val SHUFFLE = "dev.seaglass.SHUFFLE"
        const val REPEAT = "dev.seaglass.REPEAT"

        /**
         * Durée pendant laquelle le service reste au premier plan après une pause (#247),
         * comme Media3 le fait depuis la 1.6.
         */
        private const val PAUSED_HOLD_MS = 10 * 60_000L

        /** Service en cours, pour remettre à jour les boutons quand Rust change leur état. */
        var current: PlaybackService? = null
            private set
    }

    private var session: MediaSession? = null

    /** Chargeur d'images de la session : le widget y reprend la pochette déjà chargée (#329). */
    internal val bitmapLoader get() = session?.bitmapLoader

    private val main = Handler(Looper.getMainLooper())

    /** Fin du maintien au premier plan (`SystemClock.elapsedRealtime`), 0 : jamais joué. */
    private var holdUntil = 0L

    /** Maintien écoulé : la notification repasse en simple notification. */
    private val endHold = Runnable {
        session?.let { onUpdateNotification(it, false) }
    }

    /**
     * Toutes les mises à jour de la notification passent ici, avec ce que Media3 a décidé :
     * premier plan pendant la lecture seulement. En pause, il rendait donc le service à
     * l'arrière-plan ; app non visible, Android l'arrêtait une minute plus tard (« app
     * idle »), la session média était détruite et l'appui suivant sur les écouteurs
     * n'arrivait plus nulle part (#247). Le service reste donc au premier plan
     * `PAUSED_HOLD_MS` après la dernière lecture, tant qu'un titre est chargé.
     */
    override fun onUpdateNotification(session: MediaSession, startInForegroundRequired: Boolean) {
        val now = SystemClock.elapsedRealtime()
        main.removeCallbacks(endHold)
        if (startInForegroundRequired) holdUntil = now + PAUSED_HOLD_MS
        val hold = !startInForegroundRequired && now < holdUntil && resumable(session.player)
        if (hold) main.postDelayed(endHold, holdUntil - now)
        super.onUpdateNotification(session, startInForegroundRequired || hold)
    }

    /** Un titre est chargé et prêt : mêmes états que ceux où Media3 garde le premier plan. */
    private fun resumable(player: Player): Boolean = player.mediaItemCount > 0 &&
        (player.playbackState == Player.STATE_READY || player.playbackState == Player.STATE_BUFFERING)

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
            // Aucun contrôleur n'est filtré (#145, Buds et écran verrouillé) : seules les
            // commandes qu'aucun n'a à utiliser sont retirées (#254).
            val player = MediaSession.ConnectionResult.DEFAULT_PLAYER_COMMANDS.buildUpon()
                .removeAll(*RemotePlayer.BLOCKED)
                .build()
            return MediaSession.ConnectionResult.AcceptedResultBuilder(session)
                .setAvailableSessionCommands(commands)
                .setAvailablePlayerCommands(player)
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
        main.removeCallbacks(endHold)
        if (current === this) current = null
        session?.release()
        session = null
        super.onDestroy()
    }
}

/**
 * Le lecteur tel que la session le voit : lecture, pause, saut, suivant, précédent,
 * répétition et aléatoire repartent vers Rust, qui tient la file (comme SMTC sur le bureau).
 * Rien n'est appliqué directement à l'ExoPlayer, et ce que la session n'a pas à faire
 * (changer les entrées, la vitesse, le volume) n'est pas proposé aux contrôleurs (#254).
 */
@OptIn(UnstableApi::class)
class RemotePlayer(player: ExoPlayer) : ForwardingPlayer(player) {
    companion object {
        /**
         * Commandes jamais proposées : la fenêtre d'ExoPlayer (titre courant + suivant) et
         * son volume appartiennent au moteur, piloté par Rust. Retirées aussi des commandes
         * accordées aux contrôleurs dans `onConnect`.
         */
        val BLOCKED = intArrayOf(
            Player.COMMAND_SET_MEDIA_ITEM,
            Player.COMMAND_CHANGE_MEDIA_ITEMS,
            Player.COMMAND_SET_SPEED_AND_PITCH,
            Player.COMMAND_SET_VOLUME,
        )
    }

    /** Écouteurs de la session : prévenus quand Rust change la répétition ou l'aléatoire. */
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

    // Répétition et aléatoire de la file, pas de la fenêtre d'ExoPlayer : sa répétition
    // ne sert qu'à « répéter le titre » (`loop`) et ignore « répéter la file ».
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
     * État de Rust reçu (`sessionModes`, thread principal). La session ne relit pas les
     * getters : elle prend la valeur des événements. ExoPlayer n'en émet que pour sa propre
     * répétition (« répéter le titre »), avant que Rust renvoie l'état : celui-ci passe après.
     */
    fun modesChanged() {
        val repeat = repeatMode
        val shuffle = shuffleModeEnabled
        for (listener in listeners.toList()) {
            listener.onRepeatModeChanged(repeat)
            listener.onShuffleModeEnabledChanged(shuffle)
        }
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
        .removeAll(*BLOCKED)
        .build()

    override fun isCommandAvailable(command: Int): Boolean = availableCommands.contains(command)
}
