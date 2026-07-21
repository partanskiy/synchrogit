# Releasing synchrogit

Releases are tag-driven. `main` stays protected and all code changes still land through pull requests.

## Flow

1. Update `Cargo.toml` to the target version in a normal PR.
2. Merge the PR to `main`.
3. Fetch `main` locally and create a matching tag on the merged commit.
4. Push the tag.
5. Let the release workflow build and publish binary tarballs.

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

The release workflow publishes Linux tarballs for:

- `x86_64-unknown-linux-gnu`
- `aarch64-unknown-linux-gnu`

Each tarball has a flat layout:

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

Prerelease tags such as `v0.1.0-rc.1` build GitHub Release artifacts but are skipped by the AUR workflow.
