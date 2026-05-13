# synchrogit

A small daemon that keeps a set of git repositories in sync with their remotes:

- Watches each repo for local file changes and commits them (timestamp-style messages, like the `git cit` convention).
- Periodically fetches and merges remote changes.
- On a merge conflict, keeps the remote version and saves the local one alongside as `<file>.conflict-<host>-<timestamp>`, so nothing is silently lost.
- Reloads its configuration on the fly when you edit it.
- Exposes a small CLI (`status`, `reload`, `sync`) over a Unix socket.

Designed for personal sync repositories such as an Obsidian vault or a notes repo synced across multiple machines.

## Status

Early scaffolding. The current binary does nothing useful yet — actual behavior lands in subsequent PRs (see the `Milestones` section).

## Building

```sh
cargo build --locked --release
```

Requires Rust 1.94+ (edition 2024) and a recent `git` on PATH.

## License

GPL-3.0-or-later. See [`LICENSE`](LICENSE).

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md) for branch policy, commit-message conventions, and the local git hooks.
