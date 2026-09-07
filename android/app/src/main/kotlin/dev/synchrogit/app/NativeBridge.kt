package dev.synchrogit.app

import kotlinx.coroutines.asCoroutineDispatcher
import android.app.Application
import android.content.Context
import android.util.Base64
import org.json.JSONObject
import java.io.File
import java.security.KeyStore
import javax.net.ssl.TrustManagerFactory
import javax.net.ssl.X509TrustManager

object NativeBridge {
    val dispatcher = java.util.concurrent.Executors.newSingleThreadExecutor().asCoroutineDispatcher()
    init { System.loadLibrary("synchrogit") }
    @JvmStatic private external fun configure(caFile: String)
    @JvmStatic external fun call(request: String): String

    @Synchronized fun initialize(context: Context) {
        val factory = TrustManagerFactory.getInstance(TrustManagerFactory.getDefaultAlgorithm())
        factory.init(null as KeyStore?)
        val certificates = factory.trustManagers.filterIsInstance<X509TrustManager>().flatMap { it.acceptedIssuers.toList() }
        check(certificates.isNotEmpty()) { "Android trust store is empty" }
        val file = File(context.filesDir, "trusted-certificates.pem")
        file.writeText(certificates.joinToString("\n") {
            "-----BEGIN CERTIFICATE-----\n" + Base64.encodeToString(it.encoded, Base64.NO_WRAP).chunked(64).joinToString("\n") + "\n-----END CERTIFICATE-----\n"
        })
        configure(file.absolutePath)
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
