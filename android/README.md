# Android app

Kotlin + Jetpack Compose is a settings and lifecycle frontend to the same Rust
engine used on desktop. The APK includes libgit2 (HTTPS and SSH transport); the
current Android connection form supports HTTPS URLs and access tokens. It does
not require a Git app, Termux, or root. Minimum Android version: 8.0 (API 26).
The universal APK contains ARM64 and x86_64 libraries with 16 KiB ELF alignment.

## Use

1. Grant folder access for a shared folder, or use an app-private folder.
2. Enter the repository name and absolute canonical path. Supply an HTTPS URL,
   username/token for private repositories, and commit author name/email.
3. Choose **Clone** for a new empty destination, or **Use existing** for a Git
   working tree already present on the phone. Save settings.
4. Start continuous sync. The notification provides a Stop action. The app shows
   per-repository results and failures. Local edits use the Rust filesystem
   watcher; remote updates use the configured timer.

Continuous mode uses a visible dataSync foreground service. Android 15+ limits
background runtime of this service type to six hours per 24 hours. Doze and
vendor battery management can delay execution even while the service exists.
The app stops the service on timeout and explains why. It does not silently
claim to keep running. Reopen the app to restart it.

Optional **Scheduled sync** uses WorkManager (minimum interval 15 minutes,
execution can be later). It can continue after the continuous service stops and
survives process death/reboot, but it does not provide immediate file watching.
Android's Force stop disables background work until the app is opened again.

Settings use the same config.toml schema as desktop. Import/export includes
settings only. Credentials are encrypted with an Android Keystore AES-GCM key,
bound to the repository path, excluded from backups and never written to TOML.
TLS uses Android's trusted CA certificates; hostname/certificate checks remain
enabled. Do not put a token into a repository URL.

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
android/gradlew -p android assembleDebug assembleDebugAndroidTest lintDebug
android/gradlew -p android installDebug
adb shell appops set --uid dev.synchrogit.app.debug MANAGE_EXTERNAL_STORAGE allow
android/gradlew -p android connectedDebugAndroidTest
```

The fixture contains only disposable local test repositories. Instrumentation
covers the Kotlin/JNI/Rust boundary, actual Git commit/fetch/merge/push, binary
files, conflict copies, remote deletions, filesystem watching in private and
shared storage, foreground-service start/stop, Android Keystore,
HTTPS certificate validation through a public clone, and loading the Compose UI.
GitHub Actions builds the APK and runs these checks on an Android emulator.
The all-files test permission above applies to Android 11+; on Android 8–10,
grant the debug app's storage permission instead.

Release builds use ANDROID_KEYSTORE_PATH and ANDROID_KEYSTORE_PASSWORD (alias
synchrogit). All release APKs must use the same key for in-place updates through
Obtainium; debug APKs are separately signed and are for development only.
