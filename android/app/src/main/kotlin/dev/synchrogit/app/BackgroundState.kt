package dev.synchrogit.app

import android.app.ActivityManager
import android.app.ApplicationExitInfo
import android.content.Context
import android.os.Build
import java.time.Instant
import java.time.ZoneId
import java.time.format.DateTimeFormatter

/** User intent survives process death; actual engine state comes from Rust. */
class BackgroundState(context: Context) {
    private val preferences = context.getSharedPreferences("synchrogit-background", Context.MODE_PRIVATE)

    var continuousRequested: Boolean
        get() = preferences.getBoolean("continuous", false)
        set(value) { check(preferences.edit().putBoolean("continuous", value).commit()) }

    fun started() {
        preferences.edit().putLong("started_at", System.currentTimeMillis()).apply()
    }

    fun interrupted(reason: String, time: Long = System.currentTimeMillis()) {
        preferences.edit().putString("interruption", reason).putLong("interrupted_at", time).apply()
    }

    fun recordPreviousExit(context: Context) {
        if (Build.VERSION.SDK_INT < 30 || !continuousRequested) return
        val started = preferences.getLong("started_at", 0)
        if (started == 0L) return
        // Keep a known timeout/start failure instead of overwriting it when the
        // already stopped app process is subsequently reclaimed by Android.
        if (preferences.getLong("interrupted_at", 0) >= started) return
        val history = runCatching { context.getSystemService(ActivityManager::class.java)
            .getHistoricalProcessExitReasons(context.packageName, 0, 10) }.getOrNull() ?: return
        val exit = history
            .filter { it.processName == context.packageName }
            .maxByOrNull { it.timestamp } ?: return
        if (exit.timestamp < started || exit.timestamp <= preferences.getLong("interrupted_at", 0)) return
        // Persist only the reason code and time, never traces or process descriptions.
        interrupted("process:${exit.reason}", exit.timestamp)
    }

    fun interruptionText(): String? {
        val reason = preferences.getString("interruption", null) ?: return null
        val description = when (reason) {
            "timeout" -> "Android's background sync time limit was reached"
            "service_stopped" -> "The continuous sync service was stopped"
            "start_failed" -> "Continuous sync could not start"
            else -> processExitDescription(reason.removePrefix("process:").toIntOrNull())
        }
        val time = Instant.ofEpochMilli(preferences.getLong("interrupted_at", 0))
            .atZone(ZoneId.systemDefault()).format(DateTimeFormatter.ofPattern("yyyy-MM-dd HH:mm"))
        return "Last interruption ($time): $description"
    }

    private fun processExitDescription(reason: Int?): String {
        if (Build.VERSION.SDK_INT < 30) return "The app process ended"
        return when (reason) {
            ApplicationExitInfo.REASON_LOW_MEMORY -> "Android reclaimed the app's memory"
            ApplicationExitInfo.REASON_CRASH, ApplicationExitInfo.REASON_CRASH_NATIVE -> "The app process crashed"
            ApplicationExitInfo.REASON_ANR -> "Android stopped an unresponsive app process"
            ApplicationExitInfo.REASON_EXCESSIVE_RESOURCE_USAGE -> "Android stopped the app for resource use"
            ApplicationExitInfo.REASON_USER_REQUESTED, ApplicationExitInfo.REASON_USER_STOPPED -> "The app was stopped by the user or system settings"
            ApplicationExitInfo.REASON_SIGNALED -> "The app process was terminated"
            else -> "The app process ended (Android reason ${reason ?: "unknown"})"
        }
    }
}
