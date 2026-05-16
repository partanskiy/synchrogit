# synchrogit

A small daemon that keeps a set of git repositories in sync with their remotes:

- Watches each repo for local file changes and commits them (timestamp-style messages, like the `git cit` convention).
- Periodically fetches and merges remote changes.
- On a merge conflict, keeps the remote version and saves the local one alongside as `<file>.conflict-<host>-<timestamp>`, so nothing is silently lost.
- Reads a TOML config listing all repositories to supervise.

Designed for personal sync repositories such as an Obsidian vault or a notes repo synced across multiple machines.

## Status

Local development is proceeding in small PR-sized commits. The current binary supports:

- `synchrogit run`
- multi-repo TOML config loading
- per-repo filesystem watch + timer-driven sync loop
- local commit, fetch/pull, push, and keep-remote conflict handling
- Unix-socket control commands: `status`, `sync`, and a wired `reload` placeholder

Hot config reload and richer per-repo state land in later milestones.

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
commit-template = "{{date}} ({{host}})"
auto-pull = true
auto-push = true

[[repo]]
name = "notes"
path = "~/Notes/Main"

[[repo]]
name = "agent-wiki"
path = "~/.local/share/agent-wiki"
interval = "30s"
```

Repo names must be unique. Paths may use `~` and environment variables, but must be absolute after expansion.

## Usage

```sh
synchrogit run
synchrogit status
synchrogit sync
synchrogit sync notes
```

The daemon listens on `$XDG_RUNTIME_DIR/synchrogit.sock`. If `XDG_RUNTIME_DIR` is unset, it falls back to `/tmp/synchrogit-$UID.sock`. Override the path with `--socket` or `SYNCHROGIT_SOCKET`.

## Building

```sh
cargo build --locked --release
```

Requires Rust 1.94+ (edition 2024) and a recent `git` on PATH.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for branch policy, commit-message conventions, and the local git hooks.
