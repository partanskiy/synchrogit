# synchrogit

[![ci](https://img.shields.io/github/actions/workflow/status/partanskiy/synchrogit/ci.yml?branch=main&label=ci)](https://github.com/partanskiy/synchrogit/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/partanskiy/synchrogit?label=release)](https://github.com/partanskiy/synchrogit/releases/latest)
[![aur](https://img.shields.io/aur/version/synchrogit?label=aur)](https://aur.archlinux.org/packages/synchrogit)
[![brew tap](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fpartanskiy%2Fhomebrew-tap%2Fmain%2FFormula%2Fsynchrogit.rb&search=version%20%22(%5B%5E%22%5D%2B)%22&label=brew%20tap)](https://github.com/partanskiy/homebrew-tap)
[![apt](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fpartanskiy.github.io%2Fapt-repo%2FPackages&search=Version%3A%20(%5CS%2B)&label=apt)](https://github.com/partanskiy/apt-repo)
[![license](https://img.shields.io/github/license/partanskiy/synchrogit)](LICENSE)

A small daemon that keeps a set of git repositories in sync with their remotes. Point it at a repo shared between machines — an Obsidian vault, a notes folder, a wiki — and stop thinking about commits, pulls, and pushes: every machine stays a plain git clone, and the full history stays yours.

Works on **Linux, macOS and Windows**, with a **Kotlin + Compose Android app** for configuration and synchronization. The synchronization engine is shared Rust code.

## How it works

Each configured repository gets an independent worker that runs the same cycle, triggered by filesystem events (debounced, so a burst of saves collapses into one commit) and by a periodic timer:

1. **Commit** local changes with a timestamp message like `2026-07-22 17:41:03 (hostname)` (template configurable).
2. **Fetch and merge** the remote. On a merge conflict: the remote version wins in place, and your version is saved alongside as `note.conflict-<host>-<timestamp>.md` — the marker sits before the extension, so the copy stays visible in extension-filtering tools like Obsidian. If the remote deleted a file you edited locally, the original stays deleted and your edits survive in the conflict copy.
3. **Push**, but only when the remote is actually behind — an in-sync cycle touches the network once (fetch) and reports an honest `no-op`.

Around the cycle:

- **Offline-first.** Commits keep landing locally without a network; failed cycles back off exponentially, and the first successful cycle reconciles everything. Committed history remains in the repository across restarts.
- **Live control.** A local Unix socket or Windows named pipe serves `status`, `sync`, and `reload`; the config hot-reloads on edit or atomic replace, restarting only the workers whose settings actually changed.
- **Plain git underneath.** System Git is preferred, preserving its SSH configuration, agents, credential helpers and hooks. When Git is unavailable, the built-in libgit2 backend handles synchronization. Run `synchrogit backend` to see the selection; [backend details](docs/platforms.md) explain the differences.

## Installation

[![Packaging status](https://repology.org/badge/vertical-allrepos/synchrogit.svg)](https://repology.org/project/synchrogit/versions)

| OS / Distro | Package manager | Command | Version |
| --- | --- | --- | --- |
| Arch Linux | pacman ([AUR](https://aur.archlinux.org/packages/synchrogit-bin), prebuilt) | `paru -S synchrogit-bin` | ![aur bin](https://img.shields.io/aur/version/synchrogit-bin?label=) |
| Arch Linux | pacman ([AUR](https://aur.archlinux.org/packages/synchrogit), from source) | `paru -S synchrogit` | ![aur](https://img.shields.io/aur/version/synchrogit?label=) |
| macOS | [Homebrew tap](https://github.com/partanskiy/homebrew-tap) | `brew install partanskiy/tap/synchrogit` | ![brew tap](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fpartanskiy%2Fhomebrew-tap%2Fmain%2FFormula%2Fsynchrogit.rb&search=version%20%22(%5B%5E%22%5D%2B)%22&label=) |
| Debian / Ubuntu | [APT repo](https://github.com/partanskiy/apt-repo) ([setup](#debian--ubuntu)) | `sudo apt install synchrogit` | ![apt](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fpartanskiy.github.io%2Fapt-repo%2FPackages&search=Version%3A%20(%5CS%2B)&label=) |
| Fedora | dnf (COPR) | `sudo dnf copr enable partanskiy/synchrogit` | ![copr](https://img.shields.io/badge/dynamic/json?url=https%3A%2F%2Fcopr.fedorainfracloud.org%2Fapi_3%2Fpackage%3Fownername%3Dpartanskiy%26projectname%3Dsynchrogit%26packagename%3Dsynchrogit%26with_latest_build%3DTrue&query=%24.builds.latest.source_package.version&label=) |
| NixOS / any | Nix flake | `nix profile install github:partanskiy/synchrogit` | ![nix](https://img.shields.io/github/v/release/partanskiy/synchrogit?label=) |
| Any Linux | manual | `.deb` / `.rpm` / tarball from [Releases](https://github.com/partanskiy/synchrogit/releases/latest) | ![release](https://img.shields.io/github/v/release/partanskiy/synchrogit?label=) |

### Arch Linux

[`synchrogit`](https://aur.archlinux.org/packages/synchrogit) (builds from source) and [`synchrogit-bin`](https://aur.archlinux.org/packages/synchrogit-bin) (prebuilt static binary) live in the AUR:

```sh
paru -S synchrogit-bin
```

### macOS

The Homebrew tap ships a `brew services` definition, so launchd autostart is one command away:

```sh
brew install partanskiy/tap/synchrogit
brew services start synchrogit
```

### Windows

Download and extract the Windows ZIP from [Releases](https://github.com/partanskiy/synchrogit/releases/latest).
Copy its example config to `%APPDATA%\synchrogit\config.toml`, edit the repository
path, then run `synchrogit.exe run`. `Install-Startup.ps1` enables startup at login;
`Install-MinGit.ps1` optionally installs MinGit. Embedded Git is already included.

### Android

Install `synchrogit-android.apk` from [Releases](https://github.com/partanskiy/synchrogit/releases/latest),
or add this GitHub repository to Obtainium. Configure repositories in the app,
choose **Clone** or **Use existing**, save settings, then start synchronization.
The same signing key and APK name are retained across releases for updates.
The UI follows the system theme and colors. **Git defaults** shares a commit
author and SSH key across repositories; GitHub and GitLab.com server keys are
verified automatically.

Continuous sync watches local edits and checks remotes on the timer. On Android
13+, it runs without notifications in the drawer; Android's active-apps indicator
remains. On Android 8–12, its silent service notification can be hidden in system
settings. Android can suspend background execution and, on Android
15+, limits background dataSync services to six hours per day. Optional scheduled
checks run every 15 minutes or later. See the [Android guide](android/README.md)
for folder access, credentials, background behavior and building the app.

### Debian / Ubuntu

Add the signed APT repo once, then install and upgrade through `apt` as usual. The binaries are fully static, so the same packages work on any Debian or Ubuntu release:

```sh
curl -fsSL https://partanskiy.github.io/apt-repo/partanskiy.gpg | sudo tee /usr/share/keyrings/partanskiy.gpg > /dev/null
echo "deb [signed-by=/usr/share/keyrings/partanskiy.gpg] https://partanskiy.github.io/apt-repo ./" | sudo tee /etc/apt/sources.list.d/partanskiy.list
sudo apt update
sudo apt install synchrogit
```

### Fedora

```sh
sudo dnf copr enable partanskiy/synchrogit
sudo dnf install synchrogit
```

### Nix

The repository is a flake; the package includes the man page and the systemd user unit:

```sh
nix profile install github:partanskiy/synchrogit
```

### Everything else

Standalone `.deb` and `.rpm` packages and plain binary tarballs are attached to every [GitHub Release](https://github.com/partanskiy/synchrogit/releases). Linux builds are static musl binaries with an embedded Git fallback. musl targets Linux; macOS, Windows and Android use their own native targets. Every asset has a checksum, plus an aggregate `SHA256SUMS`.

## Quick start

Create a config listing the repositories to supervise:

```sh
mkdir -p ~/.config/synchrogit
$EDITOR ~/.config/synchrogit/config.toml
```

```toml
[[repo]]
name = "notes"
path = "~/Notes"
branch = "main"
remote = "origin"
```

Start the daemon — on Linux via the packaged systemd user unit, on macOS via `brew services`:

```sh
systemctl --user enable --now synchrogit   # Linux
brew services start synchrogit             # macOS
```

Check that it is syncing:

```sh
synchrogit status
```

A user service starts at login. To run at boot on headless machines (lingering) or under a root-managed system template (`synchrogit@user`), see [`docs/operations.md`](docs/operations.md).

## Configuration

On Linux/macOS, `synchrogit run` reads the first existing file from:

1. `$XDG_CONFIG_HOME/synchrogit/config.toml`
2. `~/.config/synchrogit/config.toml`
3. `/etc/synchrogit/config.toml`

Windows uses `%APPDATA%\synchrogit\config.toml` first; Android edits an app-private config through its UI.

The lookup is first-match, not a merge: a user config completely shadows the machine-wide one. Use `--config` to point at an explicit path.

```toml
[defaults]
interval = "15s"            # timer-driven sync period
debounce = "2s"             # quiet window after a filesystem event
backoff-min = "15s"         # failure backoff, doubling up to backoff-max
backoff-max = "5m"
git-timeout = "60s"
commit-template = "{ts} ({host})"
auto-pull = true
auto-push = true

[[repo]]
name = "notes"
path = "~/Notes"
branch = "main"             # guard: refuse to sync on any other branch
remote = "origin"           # explicit fetch/push remote (else the upstream)
ignore = [".direnv/**"]     # pathspec excludes; matches do not trigger commits

[[repo]]
name = "agent-wiki"
path = "~/.local/share/agent-wiki"
interval = "30s"            # per-repo override of the timer interval
```

Repo names must be unique. Paths may use `~` and environment variables, but must be absolute after expansion. Commit templates can use `{ts}` and `{host}`. Editing the config while the daemon runs is fine — it reloads automatically and keeps the previous config when the new one fails to parse.

See [`docs/config.md`](docs/config.md) for the full reference and [`examples/config.toml`](examples/config.toml) for a complete example.

## CLI

```sh
synchrogit backend   # report system/bundled Git or embedded libgit2
synchrogit run       # start the daemon in the foreground
synchrogit status    # one tab-separated line per repo
synchrogit sync      # queue an immediate cycle for all repos (or one: sync <name>)
synchrogit reload    # re-read the config now
```

On Windows, control commands use a user-specific named pipe. On Unix, they find the daemon automatically: they probe `$XDG_RUNTIME_DIR/synchrogit.sock`, the system-template socket under `/run/synchrogit/<user>/`, and the `/tmp` fallback, in that order. Override with `--socket` or `SYNCHROGIT_SOCKET`.

## Versioning

Versions follow a niri-style calendar scheme — `YY.M.PATCH`, e.g. `26.7.5` — so version numbers signal freshness rather than SemVer compatibility. Breaking changes are called out in release notes.

## Building

```sh
cargo build --locked --release
```

Requires Rust 1.94+ (edition 2024), a C compiler, make and Perl for vendored libraries. Tests require Git. Linux, macOS and Windows use this build; [Android builds](android/README.md#build-and-test) also need JDK, SDK and NDK. `--no-default-features` produces a smaller build requiring external Git.

## Packaging

Everything a distribution channel needs lives in this repository, one directory per channel, rendered and published by CI on each release:

- `packaging/aur/` — AUR `PKGBUILD` templates.
- `packaging/brew/` — Homebrew formula template for [`partanskiy/homebrew-tap`](https://github.com/partanskiy/homebrew-tap).
- `packaging/copr/` — RPM spec template for [COPR](https://copr.fedorainfracloud.org).
- `packaging/systemd/` — the user service and the per-user system template.
- `packaging/config.example.toml` — complete example config.
- `.deb`/`.rpm` metadata lives in `Cargo.toml` (`package.metadata.deb` / `package.metadata.generate-rpm`); the [APT repo](https://github.com/partanskiy/apt-repo) is regenerated from release `.deb`s.
- `flake.nix` — Nix builds straight from the repo; no publisher needed.

See [`RELEASING.md`](RELEASING.md) for the release flow.

## License

MIT. See [`LICENSE`](LICENSE). Distributed binaries also include [third-party license notices](THIRD_PARTY_LICENSES.html).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for branch policy, commit-message conventions, and the local git hooks.
