# synchrogit

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

On Arch Linux, install [`synchrogit`](https://aur.archlinux.org/packages/synchrogit) (builds from source) or [`synchrogit-bin`](https://aur.archlinux.org/packages/synchrogit-bin) (prebuilt binary) from the AUR.

On macOS, install from the Homebrew tap and manage the daemon with `brew services` (launchd):

```sh
brew install partanskiy/tap/synchrogit
brew services start synchrogit
```

Elsewhere, grab a binary tarball from [GitHub Releases](https://github.com/partanskiy/synchrogit/releases) or build from source.

## Building

```sh
cargo build --locked --release
```

Requires Rust 1.94+ (edition 2024) and a recent `git` on PATH. Linux and macOS are supported.

## Packaging

Package/install assets live under `packaging/`, one directory per distribution channel:

- `packaging/aur/` — AUR `PKGBUILD` templates, rendered and published by CI on release.
- `packaging/brew/` — Homebrew formula template for the [`partanskiy/homebrew-tap`](https://github.com/partanskiy/homebrew-tap) tap, likewise CI-published.
- `packaging/systemd/synchrogit.service` — systemd user service (Linux).
- `packaging/config.example.toml` — complete example config.

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

GPL-3.0-or-later. See [`LICENSE`](LICENSE).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for branch policy, commit-message conventions, and the local git hooks.
