package dev.synchrogit.app

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.json.JSONObject
import java.io.File
import java.util.zip.ZipInputStream

@RunWith(AndroidJUnit4::class)
class NativeSyncTest {
    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext
    private fun call(op: String, fields: JSONObject = JSONObject()) = NativeBridge.request(op, fields)

    @Test fun nativeSyncPreservesEditsConflictsAndRemoteDeletions() {
        val root = File(context.cacheDir, "sync-test-${System.nanoTime()}").apply { mkdirs() }
        try {
            val assets = InstrumentationRegistry.getInstrumentation().context.assets
            ZipInputStream(assets.open("sync-fixture.zip")).use { zip ->
                while (true) {
                    val entry = zip.nextEntry ?: break
                    val file = File(root, entry.name)
                    require(file.canonicalPath.startsWith(root.canonicalPath + "/"))
                    file.parentFile!!.mkdirs()
                    file.outputStream().use { zip.copyTo(it) }
                }
            }
            for (name in listOf("a", "b")) {
                val file = File(root, "$name/.git/config")
                file.writeText(file.readText().replace("SYNCHROGIT_TEST_REMOTE", File(root, "remote.git").absolutePath))
            }
            fun cycle(name: String) {
                val config = File(root, "$name.toml")
                config.writeText("[[repo]]\npath = ${JSONObject.quote(File(root, name).absolutePath)}\n")
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
            config.writeText("[defaults]\ninterval = '1h'\ndebounce = '50ms'\n[[repo]]\npath = ${JSONObject.quote(a.absolutePath)}\n")
            call("start", JSONObject().put("path", config.absolutePath))
            Thread.sleep(500)
            File(a, "watched.md").writeText("filesystem event\n")
            var observed = false
            for (attempt in 0 until 100) {
                Thread.sleep(100)
                // A separate one-shot cycle would mask a broken watcher, so
                // observe the worker's commit timestamp/outcome instead.
                val last = call("status").getJSONArray("repos").getJSONObject(0).getJSONObject("last_sync")
                if (last.optString("last_outcome") == "pushed") { observed = true; break }
            }
            assertTrue("local file edit should trigger the Rust watcher", observed)
            call("stop"); cycle("b")
            assertEquals("filesystem event\n", File(b, "watched.md").readText())
        } finally { call("stop"); root.deleteRecursively() }
    }

    @Test fun httpsCloneUsesAndroidTrustStore() {
        val path = File(context.cacheDir, "https-test-${System.nanoTime()}")
        try {
            call("clone", JSONObject().put("path", path.absolutePath)
                .put("url", "https://github.com/octocat/Hello-World.git").put("name", "Android Test").put("email", "test@example.com"))
            assertTrue(File(path, ".git/HEAD").exists())
            assertTrue(File(path, "README").exists())
        } finally { path.deleteRecursively() }
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
