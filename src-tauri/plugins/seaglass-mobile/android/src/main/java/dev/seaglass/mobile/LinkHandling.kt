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
 * `https://tidal.com/…` links opened in the app (#203). We do not own the domain: no
 * verification is possible, so since Android 12 the system sends them to the browser until
 * the user gives them to Seaglass in "Open by default". Before Android 12, it offers the
 * app in the usual chooser: nothing to set up.
 */
object LinkHandling {
    /** Host of TIDAL's share links; the others (`www.`, `listen.`) are rarer. */
    private const val HOST = "tidal.com"

    /** `{supported, enabled}`: `supported` is false before Android 12 (nothing to enable). */
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

    /** Opens the app's "Open by default" page (Android 12+), otherwise its info page. */
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
