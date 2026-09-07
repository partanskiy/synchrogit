package dev.synchrogit.app

import android.Manifest
import android.content.Intent
import android.net.Uri
import android.os.Build
import android.os.Bundle
import android.os.Environment
import android.provider.Settings
import androidx.activity.ComponentActivity
import androidx.activity.compose.rememberLauncherForActivityResult
import androidx.activity.compose.setContent
import androidx.activity.enableEdgeToEdge
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.lazy.items
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.text.input.PasswordVisualTransformation
import androidx.compose.ui.text.input.VisualTransformation
import androidx.compose.ui.unit.dp
import androidx.lifecycle.lifecycleScope
import kotlinx.coroutines.*
import org.json.JSONArray
import org.json.JSONObject
import java.io.File
import java.net.URI

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent { MaterialTheme { Surface(Modifier.fillMaxSize()) { SettingsScreen() } } }
    }

    @Composable private fun SettingsScreen() {
        val store = remember { SettingsStore(this) }
        var settings by remember { mutableStateOf(runCatching { store.read() }.getOrElse {
            store.message = it.message ?: "Cannot read configuration"
            JSONObject().put("defaults", JSONObject()).put("repo", JSONArray())
        }) }
        var message by remember { mutableStateOf(store.message) }
        var busy by remember { mutableStateOf(false) }
        var running by remember { mutableStateOf(false) }
        var status by remember { mutableStateOf(JSONObject()) }
        var showLicenses by remember { mutableStateOf(false) }
        var periodic by remember { mutableStateOf(store.periodic) }
        val scope = rememberCoroutineScope()
        fun perform(action: suspend () -> Unit) {
            if (busy) return
            busy = true
            scope.launch {
                try { withContext(NativeBridge.dispatcher) { action() }; message = store.message }
                catch (error: Exception) { message = error.message ?: "Operation failed"; store.message = message }
                finally { busy = false }
            }
        }
        fun update(mutator: (JSONObject) -> Unit) { settings = JSONObject(settings.toString()).also(mutator) }
        fun persist() { store.save(settings); store.applyCredentials(settings); store.message = "Settings saved" }
        val export = rememberLauncherForActivityResult(ActivityResultContracts.CreateDocument("application/toml")) { uri ->
            if (uri != null) perform {
                val source = NativeBridge.request("encode_config", JSONObject().put("settings", settings)).getString("config")
                contentResolver.openOutputStream(uri)?.use { it.write(source.toByteArray()) } ?: error("Cannot write document")
                store.message = "Configuration exported (credentials excluded)"
            }
        }
        val importLauncher = rememberLauncherForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
            if (uri != null) perform {
                val source = contentResolver.openInputStream(uri)?.bufferedReader()?.use { it.readText() } ?: error("Cannot read document")
                val decoded = NativeBridge.request("decode_config", JSONObject().put("config", source))
                withContext(Dispatchers.Main) { settings = decoded }
                store.message = "Configuration imported; review paths and save"
            }
        }
        val notifications = rememberLauncherForActivityResult(ActivityResultContracts.RequestPermission()) { }
        val storage = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { }
        LaunchedEffect(Unit) {
            while (isActive) {
                val current = withContext(NativeBridge.dispatcher) { NativeBridge.request("status") }
                status = current; running = current.optBoolean("running"); message = store.message
                delay(1000)
            }
        }
        if (showLicenses) androidx.compose.ui.window.Dialog(onDismissRequest = { showLicenses = false }) {
            Surface(shape = MaterialTheme.shapes.large) {
                Column(Modifier.fillMaxWidth().fillMaxHeight(0.9f)) {
                    androidx.compose.ui.viewinterop.AndroidView(modifier = Modifier.weight(1f), factory = { context ->
                        android.webkit.WebView(context).apply { loadUrl("file:///android_asset/THIRD_PARTY_LICENSES.html") }
                    })
                    TextButton(onClick = { showLicenses = false }) { Text("Close") }
                }
            }
        }
        val repos = settings.optJSONArray("repo") ?: JSONArray()
        LazyColumn(Modifier.fillMaxSize().safeDrawingPadding().imePadding().padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
            item {
                Spacer(Modifier.height(12.dp))
                Text("SynchroGit", style = MaterialTheme.typography.headlineLarge)
                Text("Your repositories, kept in sync", style = MaterialTheme.typography.bodyMedium)
                Spacer(Modifier.height(12.dp))
                Text(if (running) "Continuous sync is running" else "Continuous sync is stopped", style = MaterialTheme.typography.titleMedium)
                Text(message, style = MaterialTheme.typography.bodySmall)
                if (busy) LinearProgressIndicator(Modifier.fillMaxWidth())
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    Button(enabled = !busy && repos.length() > 0, onClick = {
                        if (running) { stopService(Intent(this@MainActivity, SyncService::class.java)); store.message = "Stopping synchronization…" }
                        else {
                            if (Build.VERSION.SDK_INT >= 33) notifications.launch(Manifest.permission.POST_NOTIFICATIONS)
                            perform {
                                persist()
                                withContext(Dispatchers.Main) { startForegroundService(Intent(this@MainActivity, SyncService::class.java)) }
                            }
                        }
                    }) { Text(if (running) "Stop" else "Start") }
                    OutlinedButton(enabled = running && !busy, onClick = { perform { NativeBridge.request("sync"); store.message = "Sync queued" } }) { Text("Sync now") }
                }
                val statuses = status.optJSONArray("repos") ?: JSONArray()
                for (i in 0 until statuses.length()) {
                    val repo = statuses.getJSONObject(i)
                    val last = repo.getJSONObject("last_sync")
                    Text("${repo.getString("name")}: ${last.optString("last_outcome")} — ${last.optString("last_cycle_at", "never")}", style = MaterialTheme.typography.bodySmall)
                    if (!last.isNull("last_error")) Text(last.getString("last_error"), color = MaterialTheme.colorScheme.error, style = MaterialTheme.typography.bodySmall)
                }
            }
            item {
                Text("Folder access", style = MaterialTheme.typography.titleMedium)
                Text("For shared folders such as Obsidian vaults, grant file access and enter an absolute folder path below. App-private folders need no additional access.", style = MaterialTheme.typography.bodySmall)
                OutlinedButton(onClick = {
                    if (Build.VERSION.SDK_INT >= 30) {
                        try { startActivity(Intent(Settings.ACTION_MANAGE_APP_ALL_FILES_ACCESS_PERMISSION, Uri.parse("package:$packageName"))) }
                        catch (_: android.content.ActivityNotFoundException) { startActivity(Intent(Settings.ACTION_MANAGE_ALL_FILES_ACCESS_PERMISSION)) }
                    } else storage.launch(arrayOf(Manifest.permission.READ_EXTERNAL_STORAGE, Manifest.permission.WRITE_EXTERNAL_STORAGE))
                }) { Text("Grant folder access") }
            }
            item {
                Text("Background checks", style = MaterialTheme.typography.titleMedium)
                Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) {
                    Text("Scheduled sync (15 minutes or later)", modifier = Modifier.weight(1f))
                    Switch(checked = periodic, enabled = !busy && !running, onCheckedChange = { enabled ->
                        perform {
                            if (enabled) persist()
                            ScheduledSync.configure(this@MainActivity, enabled)
                            withContext(Dispatchers.Main) { periodic = enabled }
                            store.message = if (enabled) "Scheduled synchronization enabled" else "Scheduled synchronization disabled"
                        }
                    })
                }
                Text("Continuous mode watches local edits and uses the interval below. Android may suspend it during sleep and limits background data-sync services on Android 15+ to 6 hours per day. Scheduled mode does not watch edits immediately.", style = MaterialTheme.typography.bodySmall)
            }
            item {
                Text("Defaults", style = MaterialTheme.typography.titleLarge)
                val defaults = settings.optJSONObject("defaults") ?: JSONObject()
                fun change(key: String, value: Any) = update { it.put("defaults", JSONObject(defaults.toString()).put(key, value)) }
                Field("Pull interval", defaults.optString("interval", "15s"), !running && !busy) { change("interval", it) }
                Field("Local edit debounce", defaults.optString("debounce", "2s"), !running && !busy) { change("debounce", it) }
                var advanced by remember { mutableStateOf(false) }
                TextButton(onClick = { advanced = !advanced }) { Text(if (advanced) "Hide advanced defaults" else "Advanced defaults") }
                if (advanced) {
                    for ((key, label, fallback) in listOf(Triple("backoff-min", "Minimum retry delay", "15s"), Triple("backoff-max", "Maximum retry delay", "5m"), Triple("git-timeout", "Git timeout", "60s"), Triple("commit-template", "Commit message", "{ts} ({host})"))) {
                        Field(label, defaults.optString(key, fallback), !running && !busy) { change(key, it) }
                    }
                    Toggle("Pull remote changes", defaults.optBoolean("auto-pull", true), !running && !busy) { change("auto-pull", it) }
                    Toggle("Push local commits", defaults.optBoolean("auto-push", true), !running && !busy) { change("auto-push", it) }
                    Text("Conflicts keep the remote version and save local edits in a conflict copy.", style = MaterialTheme.typography.bodySmall)
                }
            }
            items((0 until repos.length()).toList()) { index ->
                val repo = repos.getJSONObject(index)
                RepositoryEditor(repo, store, !busy && !running && !periodic,
                    onChange = { replacement -> update { it.getJSONArray("repo").put(index, replacement) } },
                    onRemove = { update { it.getJSONArray("repo").remove(index) } },
                    perform = { action -> perform(action) })
            }
            item {
                OutlinedButton(enabled = !busy && !running && !periodic, onClick = {
                    update { it.getJSONArray("repo").put(JSONObject().put("name", "repo${repos.length() + 1}").put("path", "").put("remote", "origin")) }
                }) { Text("Add repository") }
                Button(modifier = Modifier.fillMaxWidth(), enabled = !busy && !running && repos.length() > 0, onClick = { perform { persist() } }) { Text("Save settings") }
                Text("Stop continuous sync before editing settings. Turn off scheduled sync before changing repository paths or credentials.", style = MaterialTheme.typography.bodySmall)
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    TextButton(enabled = !busy && !running && !periodic, onClick = { importLauncher.launch(arrayOf("*/*")) }) { Text("Import TOML") }
                    TextButton(enabled = !busy, onClick = { export.launch("config.toml") }) { Text("Export TOML") }
                }
                TextButton(onClick = { showLicenses = true }) { Text("Open-source licenses") }
                Spacer(Modifier.height(24.dp))
            }
        }
    }

    @Composable private fun RepositoryEditor(repo: JSONObject, store: SettingsStore, enabled: Boolean, onChange: (JSONObject) -> Unit, onRemove: () -> Unit, perform: (suspend () -> Unit) -> Unit) {
        fun change(key: String, value: String) = onChange(JSONObject(repo.toString()).also { if (value.isBlank() && key !in listOf("name", "path")) it.remove(key) else it.put(key, value) })
        val path = repo.optString("path")
        var auth by remember(path) { mutableStateOf(runCatching { store.auth(path) }.getOrDefault(JSONObject())) }
        fun authChange(key: String, value: String) { auth = JSONObject(auth.toString()).put(key, value) }
        Card(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(repo.optString("name", "Repository"), style = MaterialTheme.typography.titleLarge)
                Field("Name", repo.optString("name"), enabled) { change("name", it) }
                Field("Folder path", path, enabled) { change("path", it) }
                TextButton(enabled = enabled, onClick = { change("path", File(filesDir, "repositories/${repo.optString("name", "notes")}").canonicalPath) }) { Text("Use an app-private folder") }
                Field("Branch (blank: current branch)", repo.optString("branch"), enabled) { change("branch", it) }
                Field("Remote (blank: configured upstream)", repo.optString("remote"), enabled) { change("remote", it) }
                var advanced by remember { mutableStateOf(false) }
                TextButton(onClick = { advanced = !advanced }) { Text("Repository overrides") }
                if (advanced) {
                    for (key in listOf("interval", "debounce", "commit-template")) Field(key, repo.optString(key), enabled) { change(key, it) }
                    Field("Ignore patterns (one per line)", (repo.optJSONArray("ignore") ?: JSONArray()).let { a -> (0 until a.length()).joinToString("\n") { a.getString(it) } }, enabled, multiline = true) { text ->
                        onChange(JSONObject(repo.toString()).put("ignore", JSONArray(text.lines().filter { it.isNotBlank() })))
                    }
                    for ((key, label) in listOf("auto-pull" to "Pull override", "auto-push" to "Push override")) {
                        Row { TextButton(enabled = enabled, onClick = { onChange(JSONObject(repo.toString()).also { it.remove(key) }) }) { Text("$label: inherit") }
                            Switch(checked = repo.optBoolean(key, true), enabled = enabled, onCheckedChange = { onChange(JSONObject(repo.toString()).put(key, it)) }) }
                    }
                }
                Text("Connection and commit author", style = MaterialTheme.typography.titleMedium)
                Field("HTTPS repository URL", auth.optString("url"), enabled) { authChange("url", it) }
                Field("HTTPS username", auth.optString("username", "git"), enabled) { authChange("username", it) }
                Field("Access token (blank for public repositories)", auth.optString("password"), enabled, secret = true) { authChange("password", it) }
                Field("Commit author name", auth.optString("name"), enabled) { authChange("name", it) }
                Field("Commit author email", auth.optString("email"), enabled) { authChange("email", it) }
                fun prepare(): String {
                    val absolute = File(path).also { require(it.isAbsolute) { "Folder path must be absolute" } }.canonicalPath
                    require(absolute == path) { "Use the canonical folder path: $absolute" }
                    val url = auth.optString("url")
                    if (url.isNotBlank()) {
                        val parsed = URI(url)
                        require(parsed.scheme == "https" && !parsed.host.isNullOrBlank() && parsed.userInfo == null) { "Use an HTTPS URL without credentials in it" }
                    }
                    store.saveAuth(path, auth)
                    if (url.isNotBlank()) NativeBridge.request("credentials", JSONObject().put("path", path).put("url", url).put("username", auth.optString("username", "git")).put("password", auth.optString("password")))
                    return url
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(enabled = enabled && path.isNotBlank(), onClick = { perform {
                        prepare()
                        NativeBridge.request("identity", JSONObject().put("path", path).put("name", auth.optString("name")).put("email", auth.optString("email")).put("remote", repo.optString("remote", "origin")).put("url", auth.optString("url")))
                        store.message = "Connection and author saved"
                    } }) { Text("Use existing") }
                    Button(enabled = enabled && path.isNotBlank(), onClick = { perform {
                        val url = prepare()
                        NativeBridge.request("clone", JSONObject().put("path", path).put("url", url).put("name", auth.optString("name")).put("email", auth.optString("email")))
                        store.message = "Repository cloned; save settings to begin synchronization"
                    } }) { Text("Clone") }
                }
                TextButton(enabled = enabled, onClick = onRemove) { Text("Remove from settings") }
            }
        }
    }
}

@Composable private fun Field(label: String, value: String, enabled: Boolean, secret: Boolean = false, multiline: Boolean = false, onChange: (String) -> Unit) {
    OutlinedTextField(value, onChange, modifier = Modifier.fillMaxWidth(), label = { Text(label) }, enabled = enabled,
        singleLine = !multiline, visualTransformation = if (secret) PasswordVisualTransformation() else VisualTransformation.None)
}
@Composable private fun Toggle(label: String, value: Boolean, enabled: Boolean, onChange: (Boolean) -> Unit) {
    Row(Modifier.fillMaxWidth(), horizontalArrangement = Arrangement.SpaceBetween) { Text(label, modifier = Modifier.weight(1f)); Switch(value, onChange, enabled = enabled) }
}
