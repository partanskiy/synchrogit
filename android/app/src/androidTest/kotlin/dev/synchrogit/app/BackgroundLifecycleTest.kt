package dev.synchrogit.app

import androidx.test.platform.app.InstrumentationRegistry
import androidx.work.WorkInfo
import androidx.work.WorkManager
import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Assume.assumeTrue
import org.junit.Test
import java.io.File
import java.util.concurrent.TimeUnit
import java.util.zip.ZipInputStream

class BackgroundLifecycleTest {
    private val context get() = InstrumentationRegistry.getInstrumentation().targetContext

    @Test fun stopCancelsFallbackButKeepsExplicitScheduledMode() {
        val store = SettingsStore(context)
        val background = BackgroundState(context)
        val oldPeriodic = store.periodic
        val oldRequested = background.continuousRequested
        val work = WorkManager.getInstance(context)
        fun scheduled(): Boolean = work.getWorkInfosForUniqueWork("synchrogit").get(10, TimeUnit.SECONDS)
            .any { it.state == WorkInfo.State.ENQUEUED || it.state == WorkInfo.State.RUNNING }
        fun awaitScheduled(expected: Boolean) {
            for (attempt in 0 until 100) {
                if (scheduled() == expected) return
                Thread.sleep(50)
            }
            assertEquals("Stop must preserve only the user's explicit scheduled mode", expected, scheduled())
        }
        try {
            store.periodic = false
            background.continuousRequested = true
            ScheduledSync.reconcile(context)
            awaitScheduled(true)
            SyncService.stop(context)
            awaitScheduled(false)
            assertFalse(BackgroundState(context).continuousRequested)

            ScheduledSync.configure(context, true)
            background.continuousRequested = true
            SyncService.stop(context)
            awaitScheduled(true)
            assertTrue(SettingsStore(context).periodic)
            assertFalse(BackgroundState(context).continuousRequested)
        } finally {
            store.periodic = oldPeriodic
            background.continuousRequested = oldRequested
            ScheduledSync.reconcile(context)
        }
    }

    /** Setup entry point for external background tests, not a user repository. */
    @Test fun prepareAdbScenario() {
        assumeTrue(InstrumentationRegistry.getArguments().getString("background-scenario") == "true")
        assertTrue(context.packageName.endsWith(".debug"))
        SyncService.stop(context)
        NativeBridge.request("stop")
        ScheduledSync.configure(context, false)
        val root = File(context.filesDir, "background-lifecycle-fixture")
        root.deleteRecursively()
        assertTrue(root.mkdirs())
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
        val remote = File(root, "remote.git").canonicalPath
        for (name in listOf("a", "b")) {
            val config = File(root, "$name/.git/config")
            config.writeText(config.readText().replace("SYNCHROGIT_TEST_REMOTE", remote))
        }
        SettingsStore(context).save(JSONObject()
            .put("defaults", JSONObject().put("interval", "1h").put("debounce", "50ms"))
            .put("repo", JSONArray().put(JSONObject().put("name", "Background lifecycle test")
                .put("path", File(root, "a").canonicalPath))))
    }
}
