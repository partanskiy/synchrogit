# Releasing synchrogit

Releases are tag-driven. `main` stays protected and all code changes still land through pull requests.

Versions follow a niri-style calendar scheme: `YY.M.PATCH`, e.g. `v26.7.0` for the first July 2026 release and `v26.7.1` for a patch on top of it. Mechanically these are still three-component SemVer versions (no leading zeros), so everything below applies unchanged.

## Flow

Tags go on whatever `main` commit is being released — there is no dedicated release commit. The only requirement is that `Cargo.toml` at the tagged commit already carries the target version, so land the version bump through any normal PR first (a `chore:` commit is fine, on its own or bundled with the change being released).

1. Make sure `Cargo.toml` on `main` matches the version being released.
2. Create an annotated tag on the chosen `main` commit and push it.
3. Let the release workflow build and publish binary tarballs.

```sh
git fetch origin
git switch main
git pull --ff-only origin main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

The workflow validates that:

- the tag looks like `vMAJOR.MINOR.PATCH[-pre][+build]`
- the tag version matches `Cargo.toml`
- the tagged commit is reachable from `origin/main`

## Assets

The release workflow publishes tarballs for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

Each tarball has a flat layout (`synchrogit.service` is Linux-only):

```sh
synchrogit
synchrogit.1
README.md
LICENSE
synchrogit.service
config.example.toml
```

The workflow also uploads per-asset `.sha256` files and an aggregate `SHA256SUMS`.

## AUR

After the `Release` workflow completes successfully for a pushed tag, the `Update AUR` workflow resolves the release tag from the completed workflow SHA, then renders and publishes:

- `synchrogit` from the GitHub source archive
- `synchrogit-bin` from the GitHub Release binary tarballs

The workflow needs the `AUR_SSH_PRIVATE_KEY` repository secret.

The `Update AUR` workflow can also be started by hand (workflow dispatch with a `tag` input) when the automatic chain did not run — for example after publishing release assets manually.

## Homebrew

The `Update Homebrew tap` workflow follows the same pattern: after a successful `Release` run it renders `packaging/brew/synchrogit.rb.in` with the macOS tarball URLs and checksums and pushes the formula to [`partanskiy/homebrew-tap`](https://github.com/partanskiy/homebrew-tap). It needs the `TAP_SSH_PRIVATE_KEY` repository secret (a deploy key with write access on the tap repo) and supports the same manual `workflow_dispatch` fallback.

Prerelease tags such as `v0.1.0-rc.1` build GitHub Release artifacts but are skipped by the AUR workflow.
