package dev.synchrogit.app

import android.app.*
import android.content.Context
import android.content.Intent
import android.content.pm.ServiceInfo
import android.os.IBinder
import androidx.core.app.ServiceCompat
import androidx.work.*
import kotlinx.coroutines.*
import org.json.JSONObject
import java.util.concurrent.TimeUnit

class SyncService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + NativeBridge.dispatcher)
    private var started = false
    @Volatile private var interruptionRecorded = false
    override fun onBind(intent: Intent?): IBinder? = null
    override fun onCreate() {
        super.onCreate()
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel("sync", "Synchronization", NotificationManager.IMPORTANCE_LOW).apply {
                setSound(null, null)
                enableVibration(false)
                setShowBadge(false)
            })
    }
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        val background = BackgroundState(this)
        if (intent?.action == "stop") { stop(this); return START_NOT_STICKY }
        // A sticky restart has a null intent. A queued start must not undo Stop.
        if (!background.continuousRequested) { stopSelf(); return START_NOT_STICKY }
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val stop = PendingIntent.getService(this, 1, Intent(this, SyncService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        // Android requires this even without POST_NOTIFICATIONS. On Android 13+
        // it is kept out of the notification drawer; the system's active-apps
        // indicator remains. Older versions let users hide the channel in settings.
        val notification = Notification.Builder(this, "sync").setContentTitle("SynchroGit")
            .setContentText("Watching local files and checking remotes").setSmallIcon(dev.synchrogit.app.R.drawable.ic_sync)
            .setContentIntent(open).setOngoing(true).setOnlyAlertOnce(true).setCategory(Notification.CATEGORY_SERVICE)
            .addAction(Notification.Action.Builder(null, "Stop", stop).build()).build()
        try {
            // ServiceCompat omits types unavailable before Android 14.
            ServiceCompat.startForeground(this, 1, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE)
        } catch (error: Exception) {
            // Android can still refuse a foreground restart under background restrictions.
            recordInterruption("start_failed")
            SettingsStore(this).message = error.message ?: "Continuous sync could not start; scheduled checks remain enabled"
            stopSelf()
            return START_NOT_STICKY
        }
        if (!started) {
            started = true
            scope.launch {
                val store = SettingsStore(this@SyncService)
                try {
                    store.applyCredentials()
                    NativeBridge.request("start", JSONObject().put("path", store.configFile.absolutePath))
                    background.started()
                    store.message = "Continuous synchronization is running"
                } catch (error: Exception) {
                    recordInterruption("start_failed")
                    store.message = error.message ?: "Start failed"
                    stopSelf()
                }
            }
        }
        return START_STICKY
    }
    override fun onTimeout(startId: Int, fgsType: Int) {
        // Defensive handling for platform changes; specialUse has no dataSync budget.
        recordInterruption("timeout")
        SettingsStore(this).message = "Android paused continuous sync after its time limit. Scheduled checks remain enabled; open the app to resume continuous sync."
        stopSelf()
    }
    private fun recordInterruption(reason: String) {
        interruptionRecorded = true
        BackgroundState(this).interrupted(reason)
    }
    override fun onDestroy() {
        if (BackgroundState(this).continuousRequested && !interruptionRecorded) recordInterruption("service_stopped")
        // Request native shutdown off the main thread; wait for in-flight Git
        // work before releasing workers, without an ANR on a slow connection.
        scope.launch { try { NativeBridge.request("stop") } finally { scope.cancel() } }
        super.onDestroy()
    }

    companion object {
        /** Called from a visible activity, never from a worker or boot receiver. */
        fun start(context: Context) {
            BackgroundState(context).continuousRequested = true
            ScheduledSync.reconcile(context)
            try {
                context.startForegroundService(Intent(context, SyncService::class.java))
            } catch (error: Exception) {
                BackgroundState(context).interrupted("start_failed")
                SettingsStore(context).message = error.message ?: "Continuous sync could not start; scheduled checks remain enabled"
            }
        }

        fun resumeIfRequested(context: Context) {
            if (BackgroundState(context).continuousRequested) start(context)
        }

        fun stop(context: Context) {
            BackgroundState(context).continuousRequested = false
            ScheduledSync.reconcile(context)
            context.stopService(Intent(context, SyncService::class.java))
        }
    }
}

class ScheduledSync(context: Context, parameters: WorkerParameters) : CoroutineWorker(context, parameters) {
    override suspend fun doWork(): Result = withContext(NativeBridge.dispatcher) {
        val store = SettingsStore(applicationContext)
        if (!enabled(applicationContext) || !store.configFile.exists()) return@withContext Result.success()
        try {
            // Do not replace a timeout/error message with a fictitious successful check.
            if (NativeBridge.request("status").optBoolean("running")) return@withContext Result.success()
            store.applyCredentials()
            NativeBridge.request("once", JSONObject().put("path", store.configFile.absolutePath))
            store.message = "Scheduled sync completed: ${java.time.LocalDateTime.now()}"
            Result.success()
        } catch (error: CancellationException) { throw error }
        catch (error: Exception) { store.message = error.message ?: "Scheduled sync failed"; Result.retry() }
    }
    companion object {
        fun configure(context: Context, enabled: Boolean) {
            SettingsStore(context).periodic = enabled
            reconcile(context)
        }
        fun enabled(context: Context): Boolean = SettingsStore(context).periodic || BackgroundState(context).continuousRequested

        fun reconcile(context: Context) {
            val work = WorkManager.getInstance(context)
            // KEEP preserves the next check when the activity or process reopens.
            if (enabled(context)) work.enqueueUniquePeriodicWork("synchrogit", ExistingPeriodicWorkPolicy.KEEP,
                PeriodicWorkRequestBuilder<ScheduledSync>(15, TimeUnit.MINUTES)
                    .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()).build())
            else work.cancelUniqueWork("synchrogit")
        }
    }
}
