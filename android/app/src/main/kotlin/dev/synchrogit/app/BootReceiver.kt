package dev.synchrogit.app

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent

/** Resume after the first unlock, when configuration and encrypted keys are available. */
class BootReceiver : BroadcastReceiver() {
    override fun onReceive(context: Context, intent: Intent) {
        if (intent.action == Intent.ACTION_BOOT_COMPLETED) {
            SyncService.resumeIfRequested(context)
        }
    }
}
