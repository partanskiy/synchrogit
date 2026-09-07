package dev.synchrogit.app

import android.content.Context
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.AtomicFile
import android.util.Base64
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

class SettingsStore(private val context: Context) {
    val configFile = File(context.filesDir, "config.toml")
    private val preferences = context.getSharedPreferences("synchrogit", Context.MODE_PRIVATE)
    var message: String
        get() = preferences.getString("message", "Stopped") ?: "Stopped"
        set(value) { preferences.edit().putString("message", value).apply() }
    var periodic: Boolean
        get() = preferences.getBoolean("periodic", false)
        set(value) { preferences.edit().putBoolean("periodic", value).apply() }

    fun read(): JSONObject = if (configFile.exists()) {
        NativeBridge.request("decode_config", JSONObject().put("config", configFile.readText()))
    } else JSONObject().put("defaults", JSONObject().put("interval", "15s").put("debounce", "2s"))
        .put("repo", JSONArray().put(JSONObject().put("name", "notes").put("path", "/storage/emulated/0/Notes").put("remote", "origin")))

    fun save(settings: JSONObject) {
        val text = NativeBridge.request("encode_config", JSONObject().put("settings", settings)).getString("config")
        val atomic = AtomicFile(configFile)
        val output = atomic.startWrite()
        try { output.write(text.toByteArray()); atomic.finishWrite(output) }
        catch (error: Exception) { atomic.failWrite(output); throw error }
    }

    fun auth(path: String): JSONObject {
        val saved = preferences.getString("auth:$path", null) ?: return JSONObject()
        val encrypted = JSONObject(saved)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, Base64.decode(encrypted.getString("iv"), Base64.NO_WRAP)))
        cipher.updateAAD(path.toByteArray())
        return JSONObject(String(cipher.doFinal(Base64.decode(encrypted.getString("data"), Base64.NO_WRAP))))
    }
    fun saveAuth(path: String, auth: JSONObject) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(path.toByteArray())
        val data = cipher.doFinal(auth.toString().toByteArray())
        preferences.edit().putString("auth:$path", JSONObject()
            .put("iv", Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
            .put("data", Base64.encodeToString(data, Base64.NO_WRAP)).toString()).apply()
    }
    fun applyCredentials(settings: JSONObject = read()) {
        val repos = settings.getJSONArray("repo")
        for (i in 0 until repos.length()) {
            val path = repos.getJSONObject(i).getString("path")
            val auth = auth(path)
            val url = auth.optString("url")
            if (url.isNotEmpty()) NativeBridge.request("credentials", JSONObject()
                .put("path", path).put("url", url)
                .put("username", auth.optString("username", "git")).put("password", auth.optString("password")))
        }
    }
    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey("synchrogit.credentials", null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(KeyGenParameterSpec.Builder("synchrogit.credentials", KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
            .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build())
        return generator.generateKey()
    }
}
