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
import androidx.compose.ui.platform.testTag
import androidx.compose.ui.unit.dp
import kotlinx.coroutines.*
import org.json.JSONArray
import org.json.JSONObject
import java.io.File

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        enableEdgeToEdge()
        setContent { SynchroGitTheme { Surface(Modifier.fillMaxSize()) { SettingsScreen() } } }
    }

    @Composable private fun SettingsScreen() {
        val store = remember { SettingsStore(this) }
        // Keep connection drafts while a repository card scrolls out of the
        // LazyColumn. Credentials are persisted only by explicit actions.
        val connectionDrafts = remember { mutableStateMapOf<String, JSONObject>() }
        var gitDefaults by remember { mutableStateOf(store.gitDefaults()) }
        var sshKeys by remember { mutableStateOf(store.sshKeys()) }
        var showSshKeys by remember { mutableStateOf(false) }
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
        fun persist() {
            store.saveGitDefaults(gitDefaults)
            val repos = settings.getJSONArray("repo")
            for (index in 0 until repos.length()) {
                val path = repos.getJSONObject(index).getString("path")
                connectionDrafts[path]?.let { store.saveAuth(path, it) }
                store.applyRepository(repos.getJSONObject(index), store.auth(path))
            }
            store.save(settings)
            store.applyCredentials(settings)
            store.message = "Settings saved"
        }
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
        val storage = rememberLauncherForActivityResult(ActivityResultContracts.RequestMultiplePermissions()) { }
        LaunchedEffect(Unit) {
            while (isActive) {
                val current = withContext(NativeBridge.dispatcher) { NativeBridge.request("status") }
                status = current; running = current.optBoolean("running")
                val savedMessage = store.message
                message = if (!running && savedMessage in listOf("Continuous synchronization is running", "Stopping synchronization…")) {
                    "Synchronization is stopped; press Start to resume"
                } else savedMessage
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
        if (showSshKeys) SshKeyManager(sshKeys, !busy && !running && !periodic,
            onGenerate = { name -> perform {
                val generated = store.generateSshKey(name)
                withContext(Dispatchers.Main) {
                    sshKeys = store.sshKeys()
                    if (gitDefaults.optString("ssh_key_id").isBlank()) gitDefaults = JSONObject(gitDefaults.toString()).put("ssh_key_id", generated.id)
                }
                store.message = "SSH key created; register its public key on your Git server"
            } }, onDismiss = { showSshKeys = false })
        val repos = settings.optJSONArray("repo") ?: JSONArray()
        LazyColumn(Modifier.testTag("settings-list").fillMaxSize().safeDrawingPadding().imePadding().padding(horizontal = 20.dp), verticalArrangement = Arrangement.spacedBy(12.dp)) {
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
                Toggle("Scheduled sync (15 minutes or later)", periodic, !busy && !running) { enabled ->
                    perform {
                        if (enabled) persist()
                        ScheduledSync.configure(this@MainActivity, enabled)
                        withContext(Dispatchers.Main) { periodic = enabled }
                        store.message = if (enabled) "Scheduled synchronization enabled" else "Scheduled synchronization disabled"
                    }
                }
                Text("Continuous mode watches local edits and uses the interval below. Android may suspend it during sleep and limits background data-sync services on Android 15+ to 6 hours per day. Scheduled mode does not watch edits immediately.", style = MaterialTheme.typography.bodySmall)
                if (Build.VERSION.SDK_INT < 33) {
                    Text("To hide the continuous-sync notification, turn off SynchroGit notifications in Android settings. Synchronization will continue.", style = MaterialTheme.typography.bodySmall)
                    OutlinedButton(onClick = {
                        startActivity(Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, packageName))
                    }) { Text("Android notification settings") }
                }
            }
            item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Defaults", style = MaterialTheme.typography.titleLarge)
                    Text("[defaults] · applies to every repository", style = MaterialTheme.typography.bodySmall)
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
            }
            item {
                Column(verticalArrangement = Arrangement.spacedBy(8.dp)) {
                    Text("Git defaults", style = MaterialTheme.typography.titleLarge)
                    Text("Shared author and SSH key. Repositories can override these below. Git connection settings are stored separately from config.toml.", style = MaterialTheme.typography.bodySmall)
                    fun change(key: String, value: String) { gitDefaults = JSONObject(gitDefaults.toString()).put(key, value) }
                    Field("Default commit author name", gitDefaults.optString("name"), !busy && !running && !periodic) { change("name", it) }
                    Field("Default commit author email", gitDefaults.optString("email"), !busy && !running && !periodic) { change("email", it) }
                    ChoiceField("Default SSH key", gitDefaults.optString("ssh_key_id"),
                        listOf("" to "Choose per repository") + sshKeys.map { it.id to it.name }, !busy && !running && !periodic) { change("ssh_key_id", it) }
                    OutlinedButton(onClick = { showSshKeys = true }) { Text("Manage SSH keys") }
                }
            }
            items((0 until repos.length()).toList()) { index ->
                val repo = repos.getJSONObject(index)
                RepositoryEditor(repo, settings.optJSONObject("defaults") ?: JSONObject(), gitDefaults, sshKeys, store, connectionDrafts,
                    !busy && !running && !periodic, onManageKeys = { showSshKeys = true },
                    saveGitDefaults = { store.saveGitDefaults(gitDefaults) },
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

    @Composable private fun RepositoryEditor(repo: JSONObject, defaults: JSONObject, gitDefaults: JSONObject, sshKeys: List<SshKeyInfo>, store: SettingsStore, drafts: MutableMap<String, JSONObject>, enabled: Boolean, onManageKeys: () -> Unit, saveGitDefaults: () -> Unit, onChange: (JSONObject) -> Unit, onRemove: () -> Unit, perform: (suspend () -> Unit) -> Unit) {
        fun change(key: String, value: String) = onChange(JSONObject(repo.toString()).also { if (value.isBlank() && key !in listOf("name", "path")) it.remove(key) else it.put(key, value) })
        val path = repo.optString("path")
        val auth = drafts[path] ?: remember(path) { runCatching { store.auth(path) }.getOrDefault(JSONObject()) }
        fun authChange(key: String, value: String) { drafts[path] = JSONObject(auth.toString()).put(key, value) }
        Card(Modifier.fillMaxWidth()) {
            Column(Modifier.padding(16.dp), verticalArrangement = Arrangement.spacedBy(8.dp)) {
                Text(repo.optString("name", "Repository"), style = MaterialTheme.typography.titleLarge)
                Text("[[repo]] · overrides defaults only where set", style = MaterialTheme.typography.bodySmall)
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
                        BooleanOverride(label, if (repo.has(key)) repo.getBoolean(key) else null, defaults.optBoolean(key, true), enabled) { value ->
                            onChange(JSONObject(repo.toString()).also { if (value == null) it.remove(key) else it.put(key, value) })
                        }
                    }
                }
                Text("Git connection", style = MaterialTheme.typography.titleMedium)
                val transport = connectionTransport(auth)
                ChoiceField("Authentication", transport, listOf("ssh" to "SSH key", "https" to "HTTPS token"), enabled) {
                    drafts[path] = switchTransport(auth, it)
                }
                Field("Repository URL (HTTPS or SSH)", auth.optString("url"), enabled) { authChange("url", it) }
                if (transport == "ssh") {
                    Text("git@github.com:owner/repository.git or git@gitlab.com:group/repository.git", style = MaterialTheme.typography.bodySmall)
                    val defaultKey = sshKeys.firstOrNull { it.id == gitDefaults.optString("ssh_key_id") }
                    ChoiceField("SSH key", auth.optString("ssh_key_id"),
                        listOf("" to "Use Git default (${defaultKey?.name ?: "not selected"})") + sshKeys.map { it.id to it.name }, enabled) { authChange("ssh_key_id", it) }
                    OutlinedButton(onClick = onManageKeys) { Text("Manage SSH keys") }
                    Text("A key can be shared by repositories. The Git server decides which repositories it can access.", style = MaterialTheme.typography.bodySmall)
                    Field("SSH server fingerprint (blank for GitHub or GitLab.com)", auth.optString("host_fingerprint"), enabled) { authChange("host_fingerprint", it) }
                    Text("GitHub and GitLab.com server keys are verified automatically. For a self-managed server, enter its SHA256 fingerprint from a trusted source.", style = MaterialTheme.typography.bodySmall)
                } else {
                    Field("HTTPS username", auth.optString("username", "git"), enabled) { authChange("username", it) }
                    Field("Access token (blank for public repositories)", auth.optString("password"), enabled, secret = true) { authChange("password", it) }
                }
                var authorOverride by remember(path) { mutableStateOf(auth.optString("name").isNotBlank() || auth.optString("email").isNotBlank()) }
                TextButton(onClick = { authorOverride = !authorOverride }) { Text(if (authorOverride) "Hide author overrides" else "Author overrides") }
                if (authorOverride) {
                    Field("Commit author name (blank: Git default)", auth.optString("name"), enabled) { authChange("name", it) }
                    Field("Commit author email (blank: Git default)", auth.optString("email"), enabled) { authChange("email", it) }
                }
                fun prepare(): String {
                    val absolute = File(path).also { require(it.isAbsolute) { "Folder path must be absolute" } }.canonicalPath
                    require(absolute == path) { "Use the canonical folder path: $absolute" }
                    val url = auth.optString("url")
                    saveGitDefaults()
                    store.saveAuth(path, auth)
                    store.applyConnection(path, auth)
                    return url
                }
                Row(horizontalArrangement = Arrangement.spacedBy(8.dp)) {
                    OutlinedButton(enabled = enabled && path.isNotBlank(), onClick = { perform {
                        prepare()
                        val identity = store.resolvedAuth(auth)
                        NativeBridge.request("identity", JSONObject().put("path", path).put("name", identity.optString("name")).put("email", identity.optString("email")).put("remote", repo.optString("remote", "origin")).put("url", auth.optString("url")))
                        store.message = "Connection and author saved"
                    } }) { Text("Use existing") }
                    Button(enabled = enabled && path.isNotBlank(), onClick = { perform {
                        val url = prepare()
                        val identity = store.resolvedAuth(auth)
                        NativeBridge.request("clone", JSONObject().put("path", path).put("url", url).put("name", identity.optString("name")).put("email", identity.optString("email")))
                        store.message = "Repository cloned; save settings to begin synchronization"
                    } }) { Text("Clone") }
                }
                TextButton(enabled = enabled, onClick = onRemove) { Text("Remove from settings") }
            }
        }
    }
}
