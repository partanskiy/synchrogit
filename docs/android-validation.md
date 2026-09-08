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

The final v26.9.1 APK downloaded from GitHub Release was then installed in place.
Its signature matched the existing installation, and it successfully pushed a
phone edit to GitHub and pulled a Linux reply with the saved HTTPS credentials.

With the owner's approval, that release app generated an Ed25519 key for the
existing test worktree. Only the public key was registered as a write-enabled
GitHub deploy key, restricted to that test repository. The private key stayed
in the app's encrypted storage. The existing clone's origin was changed to SSH
through **Use existing**, and GitHub's built-in host-key verification was used.
The released APK then passed these real GitHub SSH checks:

- Linux watcher → GitHub → Android timer: about 9 seconds.
- Android watcher → GitHub → Linux timer: about 11 seconds.
- Binary bytes and Unicode filenames/text survived unchanged.
- After force-stopping and reopening the app, both directions worked again
  with the saved SSH key and the app in the home-screen background (about
  22 seconds for the Linux edit and 12 seconds for the Android reply).

The Android SSH development build additionally passed all nine instrumentation
tests on the same phone. These cover private/shared storage, real Git cycles,
watcher events, foreground-service start/stop, Android TLS trust, encrypted
credentials, SSH key generation/reuse, SSH clone/fetch/push, rejection of an
incorrect SSH host fingerprint, Compose connection drafts across scrolling, and
correct service status after an activity/process restart.
The instrumentation SSH transport tests use a disposable loopback server;
the release APK checks above use the private GitHub test repository.

Long unattended operation, deep Doze, Samsung battery restrictions, reboot and
the Android 15+ six-hour dataSync service limit require separate duration tests.
USB-connected tests do not establish reliability in those conditions.

## v26.9.2 settings and shared SSH keys

The development build passed all 14 instrumentation tests on the same Samsung
SM-S936B / Android 16. New coverage verifies legacy-key migration without
changing public key material, reuse of a shared key across repository paths,
shared author defaults with local overrides, URL/authentication selection,
inheritance of a disabled pull setting, system light/dark and dynamic color
schemes, and whole-row switch interaction with a 1.8× font scale.

The physical device's system theme was dark. Visual inspection confirmed that
the new settings screen uses the system dark palette and centers the scheduled
sync switch with its label. No account key registration or change to the
existing test deploy key's server permissions is part of this update.
