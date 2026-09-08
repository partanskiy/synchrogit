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
import java.util.UUID
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

data class SshKeyInfo(val id: String, val name: String, val publicKey: String, val fingerprint: String, val migrated: Boolean)

class SettingsStore(private val context: Context) {
    val configFile = File(context.filesDir, "config.toml")
    private val preferences = context.getSharedPreferences("synchrogit", Context.MODE_PRIVATE)
    init { migrateLegacySshKeys() }
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
    fun gitDefaults(): JSONObject = readEncrypted("git-defaults", "git-defaults")
    fun saveGitDefaults(value: JSONObject) = saveEncrypted("git-defaults", "git-defaults", value)
    fun resolvedAuth(auth: JSONObject): JSONObject {
        val defaults = gitDefaults()
        return JSONObject(auth.toString()).also { resolved ->
            for (field in listOf("name", "email", "ssh_key_id")) {
                if (resolved.optString(field).isBlank()) resolved.put(field, defaults.optString(field))
            }
        }
    }
    private fun saveEncrypted(entry: String, binding: String, value: JSONObject) {
        check(preferences.edit().putString(entry, encrypted(binding, value)).commit()) { "Cannot save encrypted credentials" }
    }
    private fun encrypted(binding: String, value: JSONObject): String {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, key())
        cipher.updateAAD(binding.toByteArray())
        val data = cipher.doFinal(value.toString().toByteArray())
        return JSONObject()
            .put("iv", Base64.encodeToString(cipher.iv, Base64.NO_WRAP))
            .put("data", Base64.encodeToString(data, Base64.NO_WRAP)).toString()
    }
    private fun keyInfo(id: String, value: JSONObject) = SshKeyInfo(id, value.getString("name"),
        value.getString("public_key"), value.optString("fingerprint"), value.optBoolean("migrated"))
    fun sshKeys(): List<SshKeyInfo> = preferences.all.keys.filter { it.startsWith("ssh-key:") }.map { entry ->
        keyInfo(entry.removePrefix("ssh-key:"), readEncrypted(entry, entry))
    }.sortedBy { it.name }
    fun generateSshKey(name: String): SshKeyInfo = synchronized(keyLock) {
        require(name.isNotBlank()) { "Give the SSH key a name" }
        require(sshKeys().none { it.name == name.trim() }) { "An SSH key already has this name; choose it from the list or use another name" }
        val id = UUID.randomUUID().toString()
        val key = NativeBridge.request("generate_ssh_key").put("name", name.trim())
        saveEncrypted("ssh-key:$id", "ssh-key:$id", key)
        keyInfo(id, key)
    }
    private fun migrateLegacySshKeys() = synchronized(keyLock) {
        for (entry in preferences.all.keys.filter { it.startsWith("ssh:") }) {
            val path = entry.removePrefix("ssh:")
            val id = UUID.nameUUIDFromBytes(entry.toByteArray()).toString()
            val destination = "ssh-key:$id"
            val value = readEncrypted(entry, entry).put("name", "${File(path).name} (existing key)").put("migrated", true)
            val auth = auth(path).put("ssh_key_id", id)
            // One atomic preferences commit: never remove a working key before
            // its replacement and repository reference have both been stored.
            check(preferences.edit().putString(destination, encrypted(destination, value))
                .putString("auth:$path", encrypted(path, auth)).remove(entry).commit()) { "Cannot migrate SSH key" }
        }
    }
    fun applyConnection(path: String, auth: JSONObject) {
        val url = auth.optString("url")
        if (url.isEmpty()) return
        if (url.startsWith("https://")) {
            NativeBridge.request("credentials", JSONObject().put("path", path).put("url", url)
                .put("username", auth.optString("username", "git")).put("password", auth.optString("password")))
        } else {
            val id = resolvedAuth(auth).optString("ssh_key_id")
            val key = readEncrypted("ssh-key:$id", "ssh-key:$id")
            check(key.has("private_key")) { "Choose an SSH key in Git defaults or this repository, then register its public key on your Git server" }
            NativeBridge.request("ssh_credentials", JSONObject().put("path", path).put("url", url)
                .put("private_key", key.getString("private_key")).put("host_fingerprint", auth.optString("host_fingerprint")))
        }
    }
    fun applyRepository(repo: JSONObject, auth: JSONObject) {
        val path = repo.getString("path")
        val resolved = resolvedAuth(auth)
        val name = resolved.optString("name")
        val email = resolved.optString("email")
        // Empty author fields preserve an identity configured outside the UI.
        if (File(path, ".git").exists()) NativeBridge.request("identity", JSONObject()
            .put("path", path).put("name", name).put("email", email)
            .put("remote", repo.optString("remote", "origin")).put("url", auth.optString("url")))
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
    companion object { private val keyLock = Any() }
}
