package dev.synchrogit.app

import android.app.*
import android.content.Context
import android.content.Intent
import android.os.IBinder
import androidx.work.*
import kotlinx.coroutines.*
import org.json.JSONObject
import java.util.concurrent.TimeUnit

class SyncService : Service() {
    private val scope = CoroutineScope(SupervisorJob() + NativeBridge.dispatcher)
    private var started = false
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
        if (intent?.action == "stop") { stopSelf(); return START_NOT_STICKY }
        val open = PendingIntent.getActivity(this, 0, Intent(this, MainActivity::class.java), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val stop = PendingIntent.getService(this, 1, Intent(this, SyncService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        // Android requires this even without POST_NOTIFICATIONS. On Android 13+
        // it is kept out of the notification drawer; the system's active-apps
        // indicator remains. Older versions let users hide the channel in settings.
        startForeground(1, Notification.Builder(this, "sync").setContentTitle("SynchroGit")
            .setContentText("Watching local files and checking remotes").setSmallIcon(dev.synchrogit.app.R.drawable.ic_sync)
            .setContentIntent(open).setOngoing(true).setOnlyAlertOnce(true).setCategory(Notification.CATEGORY_SERVICE)
            .addAction(Notification.Action.Builder(null, "Stop", stop).build()).build())
        if (!started) {
            started = true
            scope.launch {
                val store = SettingsStore(this@SyncService)
                try {
                    store.applyCredentials()
                    NativeBridge.request("start", JSONObject().put("path", store.configFile.absolutePath))
                    store.message = "Continuous synchronization is running"
                } catch (error: Exception) { store.message = error.message ?: "Start failed"; stopSelf() }
            }
        }
        return START_NOT_STICKY
    }
    override fun onTimeout(startId: Int, fgsType: Int) {
        SettingsStore(this).message = "Android stopped continuous sync after its time limit. Open the app to restart; scheduled sync remains available."
        stopSelf()
    }
    override fun onDestroy() {
        // Request native shutdown off the main thread; wait for in-flight Git
        // work before releasing workers, without an ANR on a slow connection.
        scope.launch { try { NativeBridge.request("stop") } finally { scope.cancel() } }
        super.onDestroy()
    }
}

class ScheduledSync(context: Context, parameters: WorkerParameters) : CoroutineWorker(context, parameters) {
    override suspend fun doWork(): Result = withContext(NativeBridge.dispatcher) {
        val store = SettingsStore(applicationContext)
        if (!store.periodic || !store.configFile.exists()) return@withContext Result.success()
        try {
            store.applyCredentials()
            NativeBridge.request("once", JSONObject().put("path", store.configFile.absolutePath))
            store.message = "Scheduled sync completed: ${java.time.LocalDateTime.now()}"
            Result.success()
        } catch (error: Exception) { store.message = error.message ?: "Scheduled sync failed"; Result.retry() }
    }
    companion object {
        fun configure(context: Context, enabled: Boolean) {
            SettingsStore(context).periodic = enabled
            val work = WorkManager.getInstance(context)
            if (enabled) work.enqueueUniquePeriodicWork("synchrogit", ExistingPeriodicWorkPolicy.UPDATE,
                PeriodicWorkRequestBuilder<ScheduledSync>(15, TimeUnit.MINUTES)
                    .setConstraints(Constraints.Builder().setRequiredNetworkType(NetworkType.CONNECTED).build()).build())
            else work.cancelUniqueWork("synchrogit")
        }
    }
}
