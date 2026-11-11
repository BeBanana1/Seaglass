package dev.seaglass.mobile

import android.app.PendingIntent
import android.appwidget.AppWidgetManager
import android.appwidget.AppWidgetProvider
import android.content.BroadcastReceiver
import android.content.ComponentName
import android.content.Context
import android.content.Intent
import android.graphics.Bitmap
import android.net.Uri
import android.util.Log
import android.view.View
import android.widget.RemoteViews
import androidx.annotation.OptIn
import androidx.media3.common.util.UnstableApi
import androidx.media3.datasource.DataSourceBitmapLoader
import java.util.concurrent.Executor

/** Ce que le widget affiche ; `live` : la session est branchée, les boutons répondent. */
internal data class WidgetState(
    val live: Boolean = false,
    val title: String? = null,
    val artist: String? = null,
    val artwork: Uri? = null,
    val playing: Boolean = false,
)

/**
 * Widget d'écran d'accueil (#329) : pochette, titre, artiste, Précédent / Lecture-pause /
 * Suivant. Les boutons envoient les mêmes commandes que la notification (`Playback.command`,
 * donc Rust décide) ; la pochette et le texte ouvrent « En lecture ».
 *
 * Redessiné seulement quand l'état affiché change (`refresh`, appelé par `Playback`), jamais
 * sur un minuteur, et pas du tout si aucun widget n'est posé. Sans session branchée
 * (processus relancé par le système, app pas encore ouverte), le moteur ne peut pas
 * démarrer sans l'activité : le widget invite à ouvrir l'app.
 */
class NowPlayingWidget : AppWidgetProvider() {
    override fun onUpdate(context: Context, manager: AppWidgetManager, ids: IntArray) {
        Playback.onMain { refresh(context.applicationContext, force = true) }
    }

    companion object {
        private const val TAG = "SeaglassWidget"
        const val ACTION_TOGGLE = "dev.seaglass.widget.TOGGLE"
        const val ACTION_NEXT = "dev.seaglass.widget.NEXT"
        const val ACTION_PREVIOUS = "dev.seaglass.widget.PREVIOUS"

        /** Côté de la pochette envoyée au lanceur, en pixels : assez pour 72 dp. */
        private const val COVER_PX = 288

        private var shown: WidgetState? = null
        private var cover: Bitmap? = null
        private var coverUri: Uri? = null

        /** Thread principal. `force` : widget ajouté ou système qui redemande l'affichage. */
        internal fun refresh(context: Context, force: Boolean = false) {
            val manager = AppWidgetManager.getInstance(context) ?: return
            val ids = manager.getAppWidgetIds(ComponentName(context, NowPlayingWidget::class.java))
            if (ids.isEmpty()) {
                shown = null
                return
            }
            val state = Playback.widgetState()
            if (!force && state == shown) return
            shown = state
            if (state.artwork != coverUri) {
                cover = null
                coverUri = state.artwork
                state.artwork?.let { loadCover(context, it) }
            }
            try {
                manager.updateAppWidget(ids, views(context, state))
            } catch (e: RuntimeException) {
                Log.w(TAG, "widget non mis à jour : ${e.javaClass.simpleName}")
            }
        }

        private fun views(context: Context, state: WidgetState): RemoteViews {
            val views = RemoteViews(context.packageName, R.layout.widget_now_playing)
            val open = PendingIntent.getActivity(
                context,
                0,
                Intent(context, NowPlayingActivity::class.java),
                PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
            )
            views.setOnClickPendingIntent(R.id.widget_root, open)
            val known = state.live && state.title != null
            views.setTextViewText(
                R.id.widget_title,
                if (known) state.title else context.getString(R.string.widget_idle_title),
            )
            views.setTextViewText(
                R.id.widget_artist,
                if (known) state.artist.orEmpty() else context.getString(R.string.widget_idle_hint),
            )
            val bitmap = cover
            if (known && bitmap != null) {
                views.setImageViewBitmap(R.id.widget_cover, bitmap)
            } else {
                views.setImageViewResource(R.id.widget_cover, context.applicationInfo.icon)
            }
            views.setImageViewResource(
                R.id.widget_toggle,
                if (state.playing) R.drawable.widget_pause else R.drawable.widget_play,
            )
            views.setContentDescription(
                R.id.widget_toggle,
                context.getString(if (state.playing) R.string.widget_pause else R.string.widget_play),
            )
            // Sans titre connu, seul Lecture reste : il ouvre l'app.
            val skip = if (known) View.VISIBLE else View.GONE
            views.setViewVisibility(R.id.widget_previous, skip)
            views.setViewVisibility(R.id.widget_next, skip)
            views.setOnClickPendingIntent(R.id.widget_toggle, if (known) action(context, ACTION_TOGGLE) else open)
            views.setOnClickPendingIntent(R.id.widget_previous, action(context, ACTION_PREVIOUS))
            views.setOnClickPendingIntent(R.id.widget_next, action(context, ACTION_NEXT))
            return views
        }

        private fun action(context: Context, name: String): PendingIntent = PendingIntent.getBroadcast(
            context,
            0,
            Intent(context, WidgetActionReceiver::class.java).setAction(name),
            PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT,
        )

        /**
         * Pochette par le chargeur d'images de la session (celui de la notification, qui garde
         * la dernière image : pas de second téléchargement), en mémoire seulement.
         */
        @OptIn(UnstableApi::class)
        private fun loadCover(context: Context, uri: Uri) {
            val loader = PlaybackService.current?.bitmapLoader ?: DataSourceBitmapLoader(context)
            val future = try {
                loader.loadBitmap(uri)
            } catch (e: RuntimeException) {
                return
            }
            val onMain = Executor { Playback.onMain(it::run) }
            future.addListener({
                if (uri != coverUri) return@addListener
                val bitmap = try {
                    future.get()
                } catch (e: Exception) {
                    return@addListener
                }
                cover = scaled(bitmap)
                refresh(context, force = true)
            }, onMain)
        }

        /** Une image trop lourde ferait refuser la mise à jour par le lanceur. */
        private fun scaled(bitmap: Bitmap): Bitmap {
            val side = maxOf(bitmap.width, bitmap.height)
            if (side <= COVER_PX) return bitmap
            val ratio = COVER_PX.toFloat() / side
            return Bitmap.createScaledBitmap(
                bitmap,
                (bitmap.width * ratio).toInt().coerceAtLeast(1),
                (bitmap.height * ratio).toInt().coerceAtLeast(1),
                true,
            )
        }
    }
}

/**
 * Boutons du widget (#329). Non exporté : seules les diffusions de l'app y arrivent. Processus
 * relancé par l'appui (session pas branchée) : rien à commander, le widget repasse à
 * « toucher pour ouvrir ».
 */
class WidgetActionReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        val action = intent.action ?: return
        val app = context.applicationContext
        Playback.onMain {
            val state = Playback.widgetState()
            if (!state.live) return@onMain NowPlayingWidget.refresh(app, force = true)
            when (action) {
                NowPlayingWidget.ACTION_TOGGLE -> Playback.command(if (state.playing) "pause" else "play")
                NowPlayingWidget.ACTION_NEXT -> Playback.command("next")
                NowPlayingWidget.ACTION_PREVIOUS -> Playback.command("previous")
            }
        }
    }
}
