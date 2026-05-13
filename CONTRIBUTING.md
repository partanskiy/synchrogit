# Contributing to synchrogit

## Branch policy

- `main` is protected. All changes go through pull requests.
- Merge mode: **rebase only**. Squash and merge-commit modes are disabled on the GitHub side so the history stays linear and per-commit authorship is preserved.
- Force-pushing to `main` is not allowed.

## Commit messages

Two kinds of messages are accepted:

1. **Conventional Commits** for development changes:

   ```
   <type>[(scope)][!]: <description>
   ```

   Allowed types: `feat`, `fix`, `refactor`, `docs`, `test`, `chore`, `perf`, `build`, `ci`, `style`, `revert`.

2. **Release commits**:

   ```
   release: vMAJOR.MINOR.PATCH[-prerelease][+build]
   ```

   A `release: vX.Y.Z` commit **must** land together with a matching git tag `vX.Y.Z`. Push them atomically:

   ```sh
   git push --atomic origin main vX.Y.Z
   ```

   A CI check rejects `release:` commits that arrive without their tag.

Merge / revert / fixup / squash / amend commits are skipped by the linter.

## Local hooks

A `commit-msg` hook in `.githooks/` enforces the rules above. Opt in once per clone:

```sh
git config core.hooksPath .githooks
```

The same regex is enforced in CI on every commit of a pull request, so the hook is purely a convenience — but a strongly recommended one.

## Development loop

```sh
cargo fmt --all
cargo clippy --all-targets --all-features --locked -- -D warnings
cargo test --all --locked
cargo build --locked --release
```

All four must pass for CI to be green.

## Releasing

See [`RELEASING.md`](RELEASING.md) (lands with the release pipeline in a later PR).
