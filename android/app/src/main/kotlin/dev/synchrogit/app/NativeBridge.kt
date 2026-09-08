package dev.synchrogit.app

import kotlinx.coroutines.asCoroutineDispatcher
import android.app.Application
import android.content.Context
import java.io.File
import org.json.JSONObject
import java.security.KeyStore
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager

object NativeBridge {
    val dispatcher = java.util.concurrent.Executors.newSingleThreadExecutor().asCoroutineDispatcher()
    init { System.loadLibrary("synchrogit") }
    @JvmStatic private external fun configure(certificates: Array<ByteArray>, gitConfigDirectory: String)
    @JvmStatic external fun call(request: String): String

    @Synchronized fun initialize(context: Context) {
        val factory = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm())
        factory.init(null as KeyStore?)
        val certificates = factory.trustManagers.filterIsInstance<X509TrustManager>().flatMap { it.acceptedIssuers.toList() }
        check(certificates.isNotEmpty()) { "Android trust store is empty" }
        val gitConfigDirectory = File(context.noBackupFilesDir, "git").apply { mkdirs() }
        configure(certificates.map { it.encoded }.toTypedArray(), gitConfigDirectory.canonicalPath)
    }

    @Synchronized fun request(op: String, fields: JSONObject = JSONObject()): JSONObject {
        fields.put("op", op)
        val response = JSONObject(call(fields.toString()))
        check(response.getBoolean("ok")) { response.optString("error", "Native operation failed") }
        return response.getJSONObject("result")
    }
}
class SynchroGitApplication : Application() {
    override fun onCreate() { super.onCreate(); NativeBridge.initialize(this) }
}
