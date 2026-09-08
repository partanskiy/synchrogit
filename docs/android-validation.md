# Android device validation

Validated on 2026-09-08 with a Samsung SM-S936B, Android 16 (API 36), ARM64.
These results describe this device and these test conditions, not a guarantee
for every Android manufacturer's background process policy.

The installed v26.9.0 release was connected to a private GitHub test repository
using a repository-scoped HTTPS token entered by the owner. A separate Linux
daemon used the external Git backend. The phone used the embedded backend,
a shared-storage worktree, a 15-second pull interval and a 2-second debounce.

Verified through the real GitHub remote:

- Automatic Linux file edits arrived on Android without pressing Sync now.
- Android file edits arrived on Linux automatically.
- Both directions also converged with the app on the home-screen background
  and with the display off (about 26–27 seconds in the combined checks, USB connected).
- Binary contents and Unicode filenames/text were preserved byte for byte.
- Simultaneous edits kept the remote original and synchronized a copy of the
  phone's local edits back to Linux.
- A remote deletion kept the original absent on both devices and preserved the
  phone's edits only in a conflict copy. Neither repository had an unfinished merge.

An in-place update to a locally signed v26.9.1 APK retained the repository,
configuration and encrypted HTTPS token. After starting synchronization again,
the same two-way GitHub checks passed without re-entering credentials.

The Android SSH development build additionally passed all nine instrumentation
tests on the same phone. These cover private/shared storage, real Git cycles,
watcher events, foreground-service start/stop, Android TLS trust, encrypted
credentials, SSH key generation/reuse, SSH clone/fetch/push, rejection of an
incorrect SSH host fingerprint, Compose connection drafts across scrolling, and
correct service status after an activity/process restart.
SSH transport tests use a disposable loopback server, not a production account.

Long unattended operation, deep Doze, Samsung battery restrictions, reboot and
the Android 15+ six-hour dataSync service limit require separate duration tests.
USB-connected tests do not establish reliability in those conditions.
