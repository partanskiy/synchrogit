package dev.synchrogit.app

import android.content.ClipData
import android.content.ClipboardManager
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.rememberScrollState
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.foundation.verticalScroll
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.unit.dp

@Composable fun SshKeyManager(keys: List<SshKeyInfo>, enabled: Boolean, onGenerate: (String) -> Unit, onDismiss: () -> Unit) {
    val context = LocalContext.current
    var name by remember { mutableStateOf("Phone") }
    androidx.compose.ui.window.Dialog(onDismissRequest = onDismiss) {
        Surface(shape = MaterialTheme.shapes.large) {
            Column(Modifier.fillMaxWidth().heightIn(max = 640.dp).padding(20.dp)) {
                Text("SSH keys", style = MaterialTheme.typography.headlineSmall)
                Column(Modifier.weight(1f, fill = false).verticalScroll(rememberScrollState()), verticalArrangement = Arrangement.spacedBy(12.dp)) {
                    Text("Keys belong to this app and can be selected for several repositories. Add a public key to your GitHub or GitLab account for access to its repositories, or use a deploy key for limited access.", style = MaterialTheme.typography.bodySmall)
                    Text("Private keys are encrypted in this app’s private storage using Android Keystore. Only public keys can be copied. Uninstalling the app deletes its private keys.", style = MaterialTheme.typography.bodySmall)
                    keys.forEach { key ->
                        HorizontalDivider()
                        Text(key.name, style = MaterialTheme.typography.titleMedium)
                        SelectionContainer { Text(key.fingerprint, style = MaterialTheme.typography.bodySmall) }
                        if (key.migrated) Text("Preserved from an earlier version. Its permissions on the Git server are unchanged.", style = MaterialTheme.typography.bodySmall)
                        SelectionContainer { Text(key.publicKey, style = MaterialTheme.typography.bodySmall) }
                        OutlinedButton(onClick = {
                            context.getSystemService(ClipboardManager::class.java).setPrimaryClip(ClipData.newPlainText("SynchroGit SSH public key", key.publicKey))
                        }) { Text("Copy public key") }
                    }
                    HorizontalDivider()
                    Field("Key name", name, enabled) { name = it }
                    Button(enabled = enabled && name.isNotBlank(), onClick = { onGenerate(name) }) { Text("Generate SSH key") }
                    Text("Creates a new Ed25519 key on this device. Register the public key on your Git server before using it.", style = MaterialTheme.typography.bodySmall)
                }
                TextButton(onClick = onDismiss, modifier = Modifier.fillMaxWidth()) { Text("Done") }
            }
        }
    }
}

fun connectionTransport(auth: org.json.JSONObject): String {
    val url = auth.optString("url")
    return when {
        url.startsWith("https://") -> "https"
        url.startsWith("ssh://") || url.matches(Regex("[^/\\s]+@[^/\\s]+:.*")) -> "ssh"
        else -> auth.optString("transport", "ssh")
    }
}

fun switchTransport(auth: org.json.JSONObject, transport: String): org.json.JSONObject {
    val result = org.json.JSONObject(auth.toString()).put("transport", transport)
    val url = auth.optString("url")
    // Only rewrite standard GitHub/GitLab URLs. Custom servers can have
    // different SSH users, ports and paths, so require their explicit URL.
    val https = Regex("https://(github\\.com|gitlab\\.com)/([^?#]+)").matchEntire(url)
    val ssh = Regex("git@(github\\.com|gitlab\\.com):(.+)").matchEntire(url)
    when {
        transport == "ssh" && https != null -> result.put("url", "git@${https.groupValues[1]}:${https.groupValues[2]}")
        transport == "https" && ssh != null -> result.put("url", "https://${ssh.groupValues[1]}/${ssh.groupValues[2]}")
        connectionTransport(auth) != transport -> result.put("url", "")
    }
    return result
}
