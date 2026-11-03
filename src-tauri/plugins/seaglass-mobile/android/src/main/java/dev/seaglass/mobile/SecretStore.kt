package dev.seaglass.mobile

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.util.Log
import java.security.GeneralSecurityException
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Secrets de l'app (#24, refresh token TIDAL) : chiffrés en AES-256 GCM par une clé
 * générée dans l'Android Keystore, qui n'en sort jamais. Seuls l'IV et le texte chiffré
 * sont rangés dans les préférences privées de l'app. Aucune valeur n'est journalisée.
 */
internal class SecretStore(context: Context) {
    private val prefs = context.getSharedPreferences(PREFS, Context.MODE_PRIVATE)

    fun get(name: String): String? {
        val stored = prefs.getString(name, null) ?: return null
        return try {
            val (iv, data) = stored.split(':', limit = 2).map { Base64.decode(it, Base64.NO_WRAP) }
            val cipher = Cipher.getInstance(TRANSFORMATION)
            cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(TAG_BITS, iv))
            String(cipher.doFinal(data), Charsets.UTF_8)
        } catch (e: GeneralSecurityException) {
            // Clé disparue (sauvegarde restaurée sur un autre appareil…) : illisible pour
            // toujours, on l'oublie et l'utilisateur se reconnecte.
            Log.w(TAG, "secret « $name » illisible, effacé (${e.javaClass.simpleName})")
            delete(name)
            null
        } catch (e: IllegalArgumentException) {
            Log.w(TAG, "secret « $name » mal formé, effacé")
            delete(name)
            null
        }
    }

    fun set(name: String, value: String) {
        val cipher = Cipher.getInstance(TRANSFORMATION)
        cipher.init(Cipher.ENCRYPT_MODE, key())
        val data = cipher.doFinal(value.toByteArray(Charsets.UTF_8))
        val stored = Base64.encodeToString(cipher.iv, Base64.NO_WRAP) + ":" +
            Base64.encodeToString(data, Base64.NO_WRAP)
        if (!prefs.edit().putString(name, stored).commit()) {
            throw IllegalStateException("écriture du secret impossible")
        }
    }

    /** `false` si le secret n'existait pas. */
    fun delete(name: String): Boolean {
        if (!prefs.contains(name)) return false
        prefs.edit().remove(name).commit()
        return true
    }

    private fun key(): SecretKey {
        val store = KeyStore.getInstance(KEYSTORE).apply { load(null) }
        (store.getKey(KEY_ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, KEYSTORE)
        generator.init(
            KeyGenParameterSpec.Builder(
                KEY_ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }

    private companion object {
        const val TAG = "SeaglassSecrets"
        const val PREFS = "seaglass-secrets"
        const val KEYSTORE = "AndroidKeyStore"
        const val KEY_ALIAS = "seaglass-secrets"
        const val TRANSFORMATION = "AES/GCM/NoPadding"
        const val TAG_BITS = 128
    }
}
