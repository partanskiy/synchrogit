# Platforms and Git implementations

The Rust synchronization engine is shared across Linux, macOS, Windows and
Android. Local changes trigger a debounced cycle; remote changes are fetched on
the configured timer. The conflict policy is identical for both Git backends.

## Git selection

`synchrogit backend` prints the selected implementation. Selection happens once
per worker and never changes in response to an authentication or network error.

1. `SYNCHROGIT_GIT`, when set, explicitly selects a Git executable.
2. Git found on PATH (on Windows, standard Git for Windows install locations are
   checked too).
3. A bundled executable beside synchrogit: `git/cmd/git.exe` on Windows (MinGit)
   or `git/bin/git` on Unix.
4. Embedded libgit2, compiled into the standard builds.

Android uses embedded libgit2; another app's Git installation is not accessible
through its sandbox. `SYNCHROGIT_GIT_BACKEND=external` or `embedded` can force a
backend for diagnosis. An invalid or broken explicit executable fails rather
than silently switching implementations.

The external backend retains Git's configuration, credential helpers, SSH
configuration and hooks. The embedded backend supports HTTPS credential helpers
and SSH agents on desktop. It uses libgit2's certificate/host verification and
does not run Git hooks, signing commands, or arbitrary SSH configuration.
Embedded ignore patterns support ordinary Git pathspecs; advanced `:(...)`
pathspec magic requires the external backend. Network deadlines are cooperative:
a blocked native transport must finish before the next operation can start.

Build without the fallback using `cargo build --no-default-features`. Standard
builds vendor libgit2, OpenSSL and libssh2; their upstream licenses must accompany
redistributions.

## Windows

The same CLI commands work on Windows. The default configuration is
`%APPDATA%\synchrogit\config.toml`, followed by `%LOCALAPPDATA%` and
`%USERPROFILE%\.config\synchrogit\config.toml`. Use forward slashes in TOML paths,
or TOML literal strings for backslashes. Control commands use an owner-only local
named pipe; `--socket` accepts an alternate pipe name. Git subprocesses do not
open console windows.

The Windows ZIP includes an optional Install-MinGit.ps1, which downloads the
unmodified official MinGit archive and checks its pinned SHA-256. The default
Windows ZIP uses the embedded fallback immediately, without a network install.
MinGit's full original directory layout and licenses are retained when installed.
