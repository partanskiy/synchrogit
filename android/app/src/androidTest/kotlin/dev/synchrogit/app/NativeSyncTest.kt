package dev.synchrogit.app

import android.app.NotificationManager
import android.content.Intent
import android.os.Build
import android.os.Environment
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.util.zip.ZipInputStream

@RunWith(AndroidJUnit4::class)
class NativeSyncTest {
    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext
    private fun call(op: String, fields: JSONObject = JSONObject()) = NativeBridge.request(op, fields)

    @Test fun nativeSyncPreservesEditsConflictsAndRemoteDeletions() {
        syncFixture(File(context.cacheDir, "sync-test-${System.nanoTime()}"))
    }

    @Test fun sharedFolderSupportsSyncAndFileWatching() {
        if (Build.VERSION.SDK_INT >= 30) {
            assertTrue("Grant all-files access before running the shared-folder test", Environment.isExternalStorageManager())
        }
        syncFixture(File(Environment.getExternalStorageDirectory(), "synchrogit-test-${System.nanoTime()}"))
    }

    private fun syncFixture(root: File) {
        assertTrue(root.mkdirs())
        // The remote models a server, not another user-selected shared worktree.
        val remote = File(context.cacheDir, "remote-test-${System.nanoTime()}")
        val store = SettingsStore(context)
        val originalConfig = store.configFile.takeIf { it.exists() }?.readBytes()
        try {
            val assets = InstrumentationRegistry.getInstrumentation().context.assets
            ZipInputStream(assets.open("sync-fixture.zip")).use { zip ->
                while (true) {
                    val entry = zip.nextEntry ?: break
                    val file = File(root, entry.name)
                    require(file.canonicalPath.startsWith(root.canonicalPath + "/"))
                    if (entry.isDirectory) { file.mkdirs(); continue }
                    file.parentFile!!.mkdirs()
                    file.outputStream().use { zip.copyTo(it) }
                }
            }
            File(root, "remote.git").copyRecursively(remote)
            for (name in listOf("a", "b")) {
                val file = File(root, "$name/.git/config")
                file.writeText(file.readText().replace("SYNCHROGIT_TEST_REMOTE", remote.canonicalPath))
            }
            fun configText(path: File, watch: Boolean = false): String {
                val settings = JSONObject().put("repo", JSONArray().put(JSONObject().put("path", path.absolutePath)))
                if (watch) settings.put("defaults", JSONObject().put("interval", "1h").put("debounce", "50ms"))
                return call("encode_config", JSONObject().put("settings", settings)).getString("config")
            }
            fun cycle(name: String) {
                val config = File(root, "$name.toml")
                config.writeText(configText(File(root, name)))
                call("once", JSONObject().put("path", config.absolutePath))
            }
            val a = File(root, "a"); val b = File(root, "b")
            val binary = byteArrayOf(0, 1, -1, 13, 10, 42)
            File(a, "binary.bin").writeBytes(binary)
            File(a, "note.md").writeText("local edit\n")
            cycle("a"); cycle("b")
            assertEquals("local edit\n", File(b, "note.md").readText())
            assertArrayEquals(binary, File(b, "binary.bin").readBytes())

            File(b, "note.md").writeText("remote edit\n"); cycle("b")
            File(a, "note.md").writeText("conflicting local edit\n"); cycle("a")
            assertEquals("remote edit\n", File(a, "note.md").readText())
            assertTrue(a.listFiles()!!.any { it.name.startsWith("note.conflict-") && it.readText() == "conflicting local edit\n" })
            cycle("b")

            Thread.sleep(1100) // Conflict-copy timestamps have one-second precision.
            File(b, "note.md").delete(); cycle("b")
            File(a, "note.md").writeText("survives remote deletion\n"); cycle("a"); cycle("b")
            assertFalse(File(a, "note.md").exists()); assertFalse(File(b, "note.md").exists())
            assertTrue(b.listFiles()!!.any { it.name.startsWith("note.conflict-") && it.readText() == "survives remote deletion\n" })
            assertFalse(File(a, ".git/MERGE_HEAD").exists())

            val config = File(root, "watch.toml")
            config.writeText(configText(a, watch = true))
            store.configFile.writeText(config.readText())
            ActivityScenario.launch(MainActivity::class.java).use { activity ->
                activity.onActivity { it.startForegroundService(Intent(it, SyncService::class.java)) }
                for (attempt in 0 until 100) {
                    if (call("status").optBoolean("running")) break
                    Thread.sleep(100)
                }
                assertTrue("foreground service should start the Rust engine", call("status").getBoolean("running"))
                if (Build.VERSION.SDK_INT >= 33) {
                    assertFalse("continuous sync must work with drawer notifications disabled",
                        context.getSystemService(NotificationManager::class.java).areNotificationsEnabled())
                }
                Thread.sleep(500)
                File(a, "watched.md").writeText("filesystem event\n")
                var observed = false
                for (attempt in 0 until 100) {
                    Thread.sleep(100)
                    val last = call("status").getJSONArray("repos").getJSONObject(0).getJSONObject("last_sync")
                    if (last.optString("last_outcome") == "pushed") { observed = true; break }
                }
                assertTrue("local file edit should trigger the Rust watcher", observed)
                activity.onActivity { it.stopService(Intent(it, SyncService::class.java)) }
                for (attempt in 0 until 100) {
                    if (!call("status").optBoolean("running")) break
                    Thread.sleep(100)
                }
                assertFalse("stopping the service should stop the Rust engine", call("status").getBoolean("running"))
            }
            cycle("b")
            assertEquals("filesystem event\n", File(b, "watched.md").readText())
        } finally {
            context.stopService(Intent(context, SyncService::class.java))
            call("stop")
            if (originalConfig == null) store.configFile.delete() else store.configFile.writeBytes(originalConfig)
            root.deleteRecursively()
            remote.deleteRecursively()
        }
    }

    @Test fun httpsCloneUsesAndroidTrustStore() {
        for (parent in listOf(context.cacheDir, Environment.getExternalStorageDirectory())) {
            val path = File(parent, "https-test-${System.nanoTime()}").canonicalFile
            try {
                call("clone", JSONObject().put("path", path.absolutePath)
                    .put("url", "https://github.com/octocat/Hello-World.git").put("name", "Android Test").put("email", "test@example.com"))
                assertTrue(File(path, ".git/HEAD").exists())
                assertTrue(File(path, "README").exists())
            } finally { path.deleteRecursively() }
        }
    }

    @Test fun credentialsAreEncryptedAndBoundToTheRepositoryPath() {
        val store = SettingsStore(context)
        val path = File(context.filesDir, "credential-test").absolutePath
        store.saveAuth(path, JSONObject().put("password", "test-secret").put("url", "https://example.com/repo.git"))
        assertEquals("test-secret", store.auth(path).getString("password"))
        val stored = context.getSharedPreferences("synchrogit", 0).getString("auth:$path", "")!!
        assertFalse(stored.contains("test-secret"))
        assertFalse(store.auth("$path-other").has("password"))
    }

    @Test fun sharedSshKeysAreEncryptedAndUsableForDifferentRepositories() {
        val store = SettingsStore(context)
        val previousDefaults = store.gitDefaults()
        val generated = store.generateSshKey("Reusable test key")
        val prefs = context.getSharedPreferences("synchrogit", 0)
        val entry = "ssh-key:${generated.id}"
        try {
            assertTrue(generated.publicKey.startsWith("ssh-ed25519 "))
            assertTrue(runCatching { store.generateSshKey(generated.name) }.isFailure)
            val encrypted = prefs.getString(entry, "")!!
            assertFalse(encrypted.contains("PRIVATE KEY"))
            assertFalse(encrypted.contains(generated.publicKey))
            store.saveGitDefaults(JSONObject().put("ssh_key_id", generated.id).put("name", "Shared Author").put("email", "shared@example.com"))
            for (name in listOf("first", "second")) {
                val path = File(context.filesDir, "key-reuse-$name").path
                val auth = JSONObject().put("url", "git@gitlab.com:group/$name.git")
                store.applyConnection(path, auth)
                assertEquals(generated.id, store.resolvedAuth(auth).getString("ssh_key_id"))
                assertEquals("Shared Author", store.resolvedAuth(auth).getString("name"))
            }
            val override = JSONObject().put("name", "Local Author").put("ssh_key_id", generated.id)
            assertEquals("Local Author", store.resolvedAuth(override).getString("name"))
            assertEquals("shared@example.com", store.resolvedAuth(override).getString("email"))
            assertEquals(generated.publicKey, SettingsStore(context).sshKeys().single { it.id == generated.id }.publicKey)
            prefs.edit().putString("ssh-key:tampered", encrypted).commit()
            assertTrue(runCatching { store.sshKeys() }.isFailure)
        } finally {
            prefs.edit().remove(entry).remove("ssh-key:tampered").commit()
            store.saveGitDefaults(previousDefaults)
        }
    }

    @Test fun legacySshKeyMigrationPreservesKeyAndRepositorySelection() {
        val store = SettingsStore(context)
        val path = File(context.filesDir, "legacy-key-${System.nanoTime()}").canonicalPath
        val entry = "ssh:$path"
        val generated = call("generate_ssh_key")
        // Recreate the exact v26.9.1 encrypted storage format in the test app.
        store.saveAuth(path, JSONObject().put("url", "git@github.com:owner/repo.git").put("password", "legacy-token"))
        val keystore = java.security.KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        val cipher = javax.crypto.Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(javax.crypto.Cipher.ENCRYPT_MODE, keystore.getKey("synchrogit.credentials", null))
        cipher.updateAAD(entry.toByteArray())
        val encrypted = JSONObject().put("iv", android.util.Base64.encodeToString(cipher.iv, android.util.Base64.NO_WRAP))
            .put("data", android.util.Base64.encodeToString(cipher.doFinal(generated.toString().toByteArray()), android.util.Base64.NO_WRAP)).toString()
        val prefs = context.getSharedPreferences("synchrogit", 0)
        prefs.edit().putString(entry, encrypted).commit()
        val migrated = SettingsStore(context)
        val id = migrated.auth(path).getString("ssh_key_id")
        try {
            assertFalse(prefs.contains(entry))
            assertEquals("legacy-token", migrated.auth(path).getString("password"))
            val info = migrated.sshKeys().single { it.id == id }
            assertEquals(generated.getString("public_key"), info.publicKey)
            assertTrue(info.migrated)
            migrated.applyConnection(path, migrated.auth(path))
            migrated.applyConnection("$path-second", JSONObject().put("url", "git@github.com:owner/second.git").put("ssh_key_id", id))
            assertEquals(id, SettingsStore(context).auth(path).getString("ssh_key_id"))
            assertEquals(1, SettingsStore(context).sshKeys().count { it.id == id })
        } finally { prefs.edit().remove("ssh-key:$id").remove("auth:$path").remove(entry).commit() }
    }

    @Test fun sshCloneFetchPushAndHostKeyVerification() {
        val assets = InstrumentationRegistry.getInstrumentation().context.assets
        val fixture = JSONObject(assets.open("ssh-fixture.json").bufferedReader().use { it.readText() })
        val root = File(Environment.getExternalStorageDirectory(), "synchrogit-ssh-test-${System.nanoTime()}").canonicalFile
        fun credentials(path: File, fingerprint: String = fixture.getString("host_fingerprint")) {
            call("ssh_credentials", JSONObject().put("path", path.path).put("url", fixture.getString("url"))
                .put("private_key", fixture.getString("private_key")).put("host_fingerprint", fingerprint))
        }
        fun clone(path: File) = call("clone", JSONObject().put("path", path.path).put("url", fixture.getString("url"))
            .put("name", "SSH Android Test").put("email", "test@example.com"))
        fun cycle(path: File) {
            val settings = JSONObject().put("repo", JSONArray().put(JSONObject().put("path", path.path)))
            val config = File(root, "${path.name}.toml")
            config.writeText(call("encode_config", JSONObject().put("settings", settings)).getString("config"))
            call("once", JSONObject().put("path", config.path))
        }
        try {
            val rejected = File(root, "rejected")
            credentials(rejected, "SHA256:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
            val failure = runCatching { clone(rejected) }.exceptionOrNull()
            assertNotNull("An untrusted SSH host must be rejected", failure)
            assertTrue("Expected a host-key error, received ${failure?.message}", failure!!.message!!.contains("SSH server key"))
            val a = File(root, "a"); val b = File(root, "b")
            credentials(a); clone(a)
            credentials(b); clone(b)
            val marker = "SSH from Android ${System.nanoTime()}\n"
            File(a, "android.md").writeText(marker)
            cycle(a); cycle(b)
            assertEquals(marker, File(b, "android.md").readText())
            File(b, "reply.md").writeText("SSH reply\n")
            cycle(b); cycle(a)
            assertEquals("SSH reply\n", File(a, "reply.md").readText())
            assertFalse(File(a, ".git/config").readText().contains("PRIVATE KEY"))
        } finally { root.deleteRecursively() }
    }
}
