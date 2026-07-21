# synchrogit

[![ci](https://img.shields.io/github/actions/workflow/status/partanskiy/synchrogit/ci.yml?branch=main&label=ci)](https://github.com/partanskiy/synchrogit/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/partanskiy/synchrogit?label=release)](https://github.com/partanskiy/synchrogit/releases/latest)
[![aur](https://img.shields.io/aur/version/synchrogit?label=aur)](https://aur.archlinux.org/packages/synchrogit)
[![brew tap](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fraw.githubusercontent.com%2Fpartanskiy%2Fhomebrew-tap%2Fmain%2FFormula%2Fsynchrogit.rb&search=version%20%22(%5B%5E%22%5D%2B)%22&label=brew%20tap)](https://github.com/partanskiy/homebrew-tap)
[![apt](https://img.shields.io/badge/dynamic/regex?url=https%3A%2F%2Fpartanskiy.github.io%2Fapt-repo%2FPackages&search=Version%3A%20(%5CS%2B)&label=apt)](https://github.com/partanskiy/apt-repo)
[![license](https://img.shields.io/github/license/partanskiy/synchrogit)](LICENSE)

A small daemon that keeps a set of git repositories in sync with their remotes:

- Watches each repo for local file changes and commits them (timestamp-style messages, like the `git cit` convention).
- Periodically fetches and merges remote changes.
- On a merge conflict, keeps the remote version and saves the local one alongside as `<file>.conflict-<host>-<timestamp>`, so nothing is silently lost.
- Reads a TOML config listing all repositories to supervise.

Designed for personal sync repositories such as an Obsidian vault or a notes repo synced across multiple machines.

## Status

synchrogit is functional and released. Versions follow a niri-style calendar scheme (`YY.M.PATCH`, e.g. `26.7.0`), so version numbers signal freshness rather than SemVer compatibility; breaking changes are called out in release notes.

The binary supports:

- `synchrogit run`
- multi-repo TOML config loading
- per-repo filesystem watch + timer-driven sync loop
- local commit, fetch/pull, push, and keep-remote conflict handling
- Unix-socket control commands: `status`, `sync`, and `reload`
- config hot reload when `config.toml` is edited or atomically replaced
- structured per-repo status with branch, upstream, last sync result, and failure count

## Configuration

By default `synchrogit run` reads the first existing file from:

1. `$XDG_CONFIG_HOME/synchrogit/config.toml`
2. `~/.config/synchrogit/config.toml`
3. `/etc/synchrogit/config.toml`

Use `synchrogit run --config ./config.toml` to point at a specific file.

```toml
[defaults]
interval = "15s"
debounce = "2s"
backoff-min = "15s"
backoff-max = "5m"
git-timeout = "60s"
commit-template = "{ts} ({host})"
auto-pull = true
auto-push = true

[[repo]]
name = "notes"
path = "~/Notes/Main"
branch = "main"
remote = "origin"
ignore = [".direnv/**", "target/**"]

[[repo]]
name = "agent-wiki"
path = "~/.local/share/agent-wiki"
interval = "30s"
```

Repo names must be unique. Paths may use `~` and environment variables, but must be absolute after expansion. Commit templates can use `{ts}` and `{host}`.

`branch`, if set, is a guard: the daemon refuses to sync the repo when the worktree is on another branch. `remote`, if set, is used for fetch/push instead of relying on the branch upstream. `ignore` entries are passed to git as pathspec excludes for status/add, so matching files do not trigger auto-commits. `backoff-min`, `backoff-max`, and `git-timeout` must be greater than zero; `backoff-max` must be greater than or equal to `backoff-min`.

See [`docs/config.md`](docs/config.md) and [`examples/config.toml`](examples/config.toml) for the full config reference.

## Usage

```sh
synchrogit run
synchrogit status
synchrogit sync
synchrogit sync notes
synchrogit reload
```

The daemon listens on `$XDG_RUNTIME_DIR/synchrogit.sock`. If `XDG_RUNTIME_DIR` is unset, it falls back to `/tmp/synchrogit-$UID.sock`. Override the path with `--socket` or `SYNCHROGIT_SOCKET`.

`status` prints one tab-separated line per repo with the repo path, current branch, upstream, running flag, last outcome, consecutive failure count, and last error. Timer-triggered cycles use exponential backoff after failures; filesystem changes and manual `sync` commands can still trigger work immediately.

## Installation

[![Packaging status](https://repology.org/badge/vertical-allrepos/synchrogit.svg)](https://repology.org/project/synchrogit/versions)

| Platform | Install |
| --- | --- |
| Arch Linux | `paru -S synchrogit-bin` (prebuilt) or `paru -S synchrogit` (from source) |
| macOS | `brew install partanskiy/tap/synchrogit` |
| Debian/Ubuntu | signed APT repo — [see below](#debianubuntu) |
| Fedora | `sudo dnf copr enable partanskiy/synchrogit && sudo dnf install synchrogit` |
| Nix | `nix profile install github:partanskiy/synchrogit` |
| Anything else | static `.deb` / `.rpm` / tarball from [Releases](https://github.com/partanskiy/synchrogit/releases/latest) |

### Arch Linux

[`synchrogit`](https://aur.archlinux.org/packages/synchrogit) (builds from source) and [`synchrogit-bin`](https://aur.archlinux.org/packages/synchrogit-bin) (prebuilt static binary) live in the AUR:

```sh
paru -S synchrogit-bin
systemctl --user enable --now synchrogit
```

### macOS

The Homebrew tap ships a `brew services` definition, so launchd autostart is one command away:

```sh
brew install partanskiy/tap/synchrogit
brew services start synchrogit
```

### Debian/Ubuntu

Add the signed APT repo once, then install and upgrade through `apt` as usual:

```sh
curl -fsSL https://partanskiy.github.io/apt-repo/partanskiy.gpg | sudo tee /usr/share/keyrings/partanskiy.gpg > /dev/null
echo "deb [signed-by=/usr/share/keyrings/partanskiy.gpg] https://partanskiy.github.io/apt-repo ./" | sudo tee /etc/apt/sources.list.d/partanskiy.list
sudo apt update
sudo apt install synchrogit
systemctl --user enable --now synchrogit
```

The binaries are fully static, so the same packages work on any Debian or Ubuntu release.

### Fedora

```sh
sudo dnf copr enable partanskiy/synchrogit
sudo dnf install synchrogit
systemctl --user enable --now synchrogit
```

### Nix

The repository is a flake; the package includes the man page and the systemd user unit:

```sh
nix profile install github:partanskiy/synchrogit
```

### Everything else

Standalone `.deb` and `.rpm` packages and plain binary tarballs are attached to every [GitHub Release](https://github.com/partanskiy/synchrogit/releases) (Linux builds are static musl binaries that run on any distro), with per-file checksums and an aggregate `SHA256SUMS`.

To start the daemon at boot on headless machines — lingering and the `synchrogit@user` system template — see [`docs/operations.md`](docs/operations.md).

## Building

```sh
cargo build --locked --release
```

Requires Rust 1.94+ (edition 2024) and a recent `git` on PATH. Linux and macOS are supported.

## Packaging

Package/install assets live under `packaging/`, one directory per distribution channel:

- `packaging/aur/` — AUR `PKGBUILD` templates, rendered and published by CI on release.
- `packaging/brew/` — Homebrew formula template for the [`partanskiy/homebrew-tap`](https://github.com/partanskiy/homebrew-tap) tap, likewise CI-published.
- `packaging/copr/` — RPM spec template submitted to [COPR](https://copr.fedorainfracloud.org) by CI.
- `packaging/systemd/` — systemd user service and the per-user system template (Linux).
- `packaging/config.example.toml` — complete example config.
- `.deb`/`.rpm` metadata lives in `Cargo.toml` (`package.metadata.deb` / `package.metadata.generate-rpm`); the [APT repo](https://github.com/partanskiy/apt-repo) is regenerated from release `.deb`s by CI.
- `flake.nix` — Nix builds straight from the repo; no publisher needed.

For a manual local install, copy the example config to `~/.config/synchrogit/config.toml`, install the service, and make sure `ExecStart` points at the installed binary path. The packaged unit assumes `/usr/bin/synchrogit`.

```sh
mkdir -p ~/.config/synchrogit ~/.config/systemd/user
cp packaging/config.example.toml ~/.config/synchrogit/config.toml
cp packaging/systemd/synchrogit.service ~/.config/systemd/user/synchrogit.service
$EDITOR ~/.config/systemd/user/synchrogit.service
systemctl --user daemon-reload
systemctl --user enable --now synchrogit.service
```

See [`docs/operations.md`](docs/operations.md) for day-to-day commands and release notes.

## License

MIT. See [`LICENSE`](LICENSE).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for branch policy, commit-message conventions, and the local git hooks.
