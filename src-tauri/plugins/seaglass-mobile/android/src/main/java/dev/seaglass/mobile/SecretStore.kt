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
 * The app's secrets (#24, TIDAL refresh token): encrypted with AES-256 GCM by a key
 * generated in the Android Keystore, which never leaves it. Only the IV and the ciphertext
 * are stored in the app's private preferences. No value is logged.
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
            // Key gone (backup restored on another device...): unreadable forever, so we
            // forget it and the user signs in again.
            Log.w(TAG, "secret \"$name\" unreadable, erased (${e.javaClass.simpleName})")
            delete(name)
            null
        } catch (e: IllegalArgumentException) {
            Log.w(TAG, "secret \"$name\" malformed, erased")
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
            throw IllegalStateException("cannot write the secret")
        }
    }

    /** `false` if the secret did not exist. */
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
