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
        return readEncrypted("auth:$path", path)
    }
    private fun readEncrypted(entry: String, binding: String): JSONObject {
        val saved = preferences.getString(entry, null) ?: return JSONObject()
        val encrypted = JSONObject(saved)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, Base64.decode(encrypted.getString("iv"), Base64.NO_WRAP)))
        cipher.updateAAD(binding.toByteArray())
        return JSONObject(String(cipher.doFinal(Base64.decode(encrypted.getString("data"), Base64.NO_WRAP))))
    }
    fun saveAuth(path: String, auth: JSONObject) {
        saveEncrypted("auth:$path", path, auth)
    }
    private fun saveEncrypted(entry: String, binding: String, value: JSONObject) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(binding.toByteArray())
        val data = cipher.doFinal(value.toString().toByteArray())
        check(preferences.edit().putString(entry, JSONObject()
            .put("iv", Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
            .put("data", Base64.encodeToString(data, Base64.NO_WRAP)).toString()).commit()) { "Cannot save encrypted credentials" }
    }
    fun sshPublicKey(path: String): String = readEncrypted("ssh:$path", "ssh:$path").optString("public_key")
    fun generateSshKey(path: String): String {
        require(File(path).isAbsolute && File(path).canonicalPath == path) { "Choose a canonical repository folder before generating its SSH key" }
        val existing = sshPublicKey(path)
        if (existing.isNotEmpty()) return existing
        val key = NativeBridge.request("generate_ssh_key")
        saveEncrypted("ssh:$path", "ssh:$path", key)
        return key.getString("public_key")
    }
    fun applyConnection(path: String, auth: JSONObject) {
        val url = auth.optString("url")
        if (url.isEmpty()) return
        if (url.startsWith("https://")) {
            NativeBridge.request("credentials", JSONObject().put("path", path).put("url", url)
                .put("username", auth.optString("username", "git")).put("password", auth.optString("password")))
        } else {
            val key = readEncrypted("ssh:$path", "ssh:$path")
            check(key.has("private_key")) { "Generate an SSH key for this repository and add its public key to your Git server first" }
            NativeBridge.request("ssh_credentials", JSONObject().put("path", path).put("url", url)
                .put("private_key", key.getString("private_key")).put("host_fingerprint", auth.optString("host_fingerprint")))
        }
    }
    fun applyCredentials(settings: JSONObject = read()) {
        val repos = settings.getJSONArray("repo")
        for (i in 0 until repos.length()) {
            val path = repos.getJSONObject(i).getString("path")
            val auth = auth(path)
            applyConnection(path, auth)
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
