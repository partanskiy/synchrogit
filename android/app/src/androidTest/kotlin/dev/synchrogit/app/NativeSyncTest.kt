package dev.synchrogit.app

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
}
