package dev.seaglass.mobile

import android.content.Context
import android.content.Intent
import android.content.pm.verify.domain.DomainVerificationManager
import android.content.pm.verify.domain.DomainVerificationUserState
import android.net.Uri
import android.os.Build
import android.provider.Settings
import org.json.JSONObject

/**
 * Liens `https://tidal.com/…` ouverts dans l'app (#203). Le domaine n'est pas à nous : pas de
 * vérification possible, donc depuis Android 12 le système les envoie au navigateur tant que
 * l'utilisateur ne les a pas donnés à Seaglass dans « Ouvrir par défaut ». Avant Android 12,
 * il propose l'app dans le choix habituel : rien à régler.
 */
object LinkHandling {
    /** Hôte des liens de partage de TIDAL ; les autres (`www.`, `listen.`) sont plus rares. */
    private const val HOST = "tidal.com"

    /** `{supported, enabled}` : `supported` faux avant Android 12 (rien à activer). */
    fun state(context: Context): JSONObject {
        val result = JSONObject()
        if (Build.VERSION.SDK_INT < Build.VERSION_CODES.S) {
            return result.put("supported", false).put("enabled", true)
        }
        val manager = context.getSystemService(DomainVerificationManager::class.java)
        val user = manager?.getDomainVerificationUserState(context.packageName)
        val host = user?.hostToStateMap?.get(HOST)
        val enabled = user != null && user.isLinkHandlingAllowed &&
            (host == DomainVerificationUserState.DOMAIN_STATE_SELECTED ||
                host == DomainVerificationUserState.DOMAIN_STATE_VERIFIED)
        return result.put("supported", true).put("enabled", enabled)
    }

    /** Ouvre « Ouvrir par défaut » de l'app (Android 12+), sinon sa page d'infos. */
    fun openSettings(context: Context) {
        val uri = Uri.parse("package:${context.packageName}")
        val action = if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.S) {
            Settings.ACTION_APP_OPEN_BY_DEFAULT_SETTINGS
        } else {
            Settings.ACTION_APPLICATION_DETAILS_SETTINGS
        }
        context.startActivity(Intent(action, uri).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
    }
}
