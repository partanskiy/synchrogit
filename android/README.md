# Android app

Kotlin + Jetpack Compose is a settings and lifecycle frontend to the same Rust
engine used on desktop. The APK includes libgit2 and supports HTTPS access
tokens and SSH keys generated on the device. It does
not require a Git app, Termux, or root. Minimum Android version: 8.0 (API 26).
The universal APK contains ARM64 and x86_64 libraries with 16 KiB ELF alignment.

## Use

1. Grant folder access for a shared folder, or use an app-private folder.
2. Set a shared commit author and SSH key in **Git defaults**. Enter the repository
   name and absolute canonical path, choose SSH or HTTPS, and supply its URL.
   A repository can override the shared author or select another SSH key.
3. Choose **Clone** for a new empty destination, or **Use existing** for a Git
   working tree already present on the phone. Save settings.
4. Start continuous sync; use **Stop** in the app to stop it. The app shows
   per-repository results and failures. Local edits use the Rust filesystem
   watcher; remote updates use the configured timer.

Continuous mode uses a dataSync foreground service. On Android 13+, the app does
not request notification permission and does not show notifications in the drawer.
Android can still list it in **Active apps** while the service runs. On Android
8–12, the required service notification is silent; use **Android notification
settings** under **Background checks** to hide it without stopping synchronization.
Android 15+ limits
background runtime of this service type to six hours per 24 hours. Doze and
vendor battery management can delay execution even while the service exists.
The app stops the service on timeout and explains why. It does not silently
claim to keep running. Reopen the app to restart it.

Optional **Scheduled sync** uses WorkManager (minimum interval 15 minutes,
execution can be later). It can continue after the continuous service stops and
survives process death/reboot, but it does not provide immediate file watching.
Android's Force stop disables background work until the app is opened again.

The app follows the system light/dark theme and uses the system's dynamic colors
on Android 12 and later. Earlier versions use matching Material light/dark colors.

Synchronization settings use the same `config.toml` schema as desktop:
**Defaults** maps to `[defaults]`, and each repository maps to `[[repo]]`.
Pull and push overrides offer **Use defaults**, **On**, and **Off**; inheritance
shows the actual default and removes the override from exported TOML.

**Git defaults** provides a shared commit author and a default SSH key. Author
fields left blank in a repository inherit these values; saving applies the
resolved author to existing Git worktrees. Leaving both shared and local author
fields blank preserves a manually configured Git identity. Repository URLs,
credentials, author preferences and key selections are stored separately from
SynchroGit's TOML, like desktop Git authentication and author configuration.
Import/export contains synchronization settings only. HTTPS tokens remain
bound to the repository path; SSH keys are shared through explicit selections.
TLS uses Android's trusted CA certificates. Do not put a token into a URL.

### SSH authentication

Open **Git defaults → Manage SSH keys**, give a key a recognizable name, and
choose **Generate SSH key**. The Rust core generates an Ed25519 key using the
operating system's random source. The key belongs to the app and can be selected
for multiple repositories; generating another named key creates a distinct key.
Choose a default in **Git defaults**, or choose a specific key in a repository.

**Copy public key** and register it on your Git server. An account SSH key on
GitHub or GitLab can access that account's repositories. A deploy key can provide
more limited access; GitHub deploy keys are restricted to a single repository.
The app's ability to reuse a key does not expand permissions on the server.

Choose **SSH key** authentication and enter `git@github.com:owner/repository.git`
or `git@gitlab.com:group/repository.git`. Switching between SSH and HTTPS converts
standard GitHub/GitLab URLs; custom server URLs must be entered explicitly.
Then clone, or choose **Use existing** for a working tree already on the phone,
and save settings. Saving also applies connection changes to existing worktrees.

Private keys are encrypted with an Android Keystore AES-GCM key and supplied to
libgit2 only in memory. They are not hardware SSH keys themselves. Encrypted
records live in the app's private `shared_prefs/synchrogit.xml`, bound to stable
key IDs rather than repository paths. On a typical device the release app's
private data directory is `/data/user/0/dev.synchrogit.app/`. Private keys never
enter a repository, TOML export, clipboard, or backup; uninstalling deletes them.

When upgrading from v26.9.1, existing repository keys are atomically moved to the
shared key list, preserving the key material and each repository's selection.
They are labelled as existing keys; server-side permissions are unchanged, and
no account registration or new key generation happens during migration.

Server identity is a separate check from your client authentication key. Pins
verified on 2026-09-08 are included for:

- `github.com:22` and `ssh.github.com:443`, from
  [GitHub's published fingerprints](https://docs.github.com/en/authentication/keeping-your-account-and-data-secure/githubs-ssh-key-fingerprints).
- `gitlab.com:22` and `altssh.gitlab.com:443`, from
  [GitLab's published fingerprints](https://docs.gitlab.com/user/gitlab_com/#ssh-host-keys-fingerprints)
  and [live instance configuration](https://gitlab.com/help/instance_configuration).

Self-managed GitLab and other servers need an explicit `SHA256:...` fingerprint
from their administrator over a trusted channel. Unknown or changed server keys
are rejected; the app never automatically trusts the first server that answers.

For HTTPS, enter the username and access token; public repositories can leave
the token blank. GitHub private repositories can use a fine-grained token
restricted to that repository with **Contents: Read and write**.

Removing a repository from settings leaves its files intact. Uninstalling the
app deletes app-private repositories and credentials; use shared folders if
other editors need direct access to the files.

Shared storage reports ownership differently from private app storage. The app
adds explicitly selected repository paths to its private Git `safe.directory`
list; libgit2 ownership verification remains enabled.

## Build and test

Install Rust Android targets, JDK 21, SDK 36, build-tools 35.0.0 and NDK
28.2.13676358. Set ANDROID_HOME, then from the project root:

```sh
rustup target add aarch64-linux-android x86_64-linux-android
scripts/build-android.sh
python3 scripts/android-test-fixture.py
python3 scripts/android-ssh-fixture.py
android/gradlew -p android assembleDebug assembleDebugAndroidTest lintDebug
android/gradlew -p android installDebug
adb shell appops set --uid dev.synchrogit.app.debug MANAGE_EXTERNAL_STORAGE allow
adb reverse tcp:22229 tcp:22229
android/gradlew -p android connectedDebugAndroidTest
adb reverse --remove tcp:22229
python3 scripts/android-ssh-fixture.py --stop
```

The fixture contains only disposable local test repositories. Instrumentation
covers the Kotlin/JNI/Rust boundary, actual Git commit/fetch/merge/push, binary
files, conflict copies, remote deletions, filesystem watching in private and
shared storage, foreground-service start/stop without notification permission on
Android 13+, Android Keystore,
HTTPS certificate validation through a public clone, and loading the Compose UI.
It also checks real SSH clone/fetch/push, rejection of an incorrect server key,
encrypted shared SSH keys, migration of legacy keys, reusable author defaults,
system dark/light and dynamic colors, large-font switch interaction, explicit
authentication selection, and three-state overrides. The SSH fixture requires a Linux
host with OpenSSH server installed. It listens only on loopback, authorizes a
disposable key, and restricts commands to its temporary Git repository; it does
not use the developer's SSH keys. Remove `target/android-ssh-fixture` before
starting a new fixture after stopping the previous server.
GitHub Actions builds the APK and runs these checks on an Android emulator.
The all-files test permission above applies to Android 11+; on Android 8–10,
grant the debug app's storage permission instead.

Release builds use ANDROID_KEYSTORE_PATH and ANDROID_KEYSTORE_PASSWORD (alias
synchrogit). All release APKs must use the same key for in-place updates through
Obtainium; debug APKs are separately signed and are for development only.
