# Releasing synchrogit

Releases are tag-driven. `main` stays protected and all code changes still land through pull requests.

Versions follow a niri-style calendar scheme: `YY.M.PATCH`, e.g. `v26.7.0` for the first July 2026 release and `v26.7.1` for a patch on top of it. Mechanically these are still three-component SemVer versions (no leading zeros), so everything below applies unchanged.

## Flow

Tags go on whatever `main` commit is being released — there is no dedicated release commit. The only requirement is that `Cargo.toml` at the tagged commit already carries the target version, so land the version bump through any normal PR first (a `chore:` commit is fine, on its own or bundled with the change being released).

1. Make sure `Cargo.toml` on `main` matches the version being released.
2. Create an annotated tag on the chosen `main` commit and push it.
3. Let the release workflow build and publish desktop archives, Linux packages and the signed Android APK.
4. Confirm Update AUR (both packages), Update APT repo and Update Homebrew tap succeed; check the latest COPR build too.

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

- `x86_64-unknown-linux-musl`
- `aarch64-unknown-linux-musl`
- `aarch64-apple-darwin`
- `x86_64-apple-darwin`

Each tarball has a flat layout (the systemd units are Linux-only):

```sh
synchrogit
synchrogit.1
README.md
LICENSE
THIRD_PARTY_LICENSES.html
synchrogit.service
synchrogit@.service
config.example.toml
```

The workflow also uploads per-asset `.sha256` files and an aggregate `SHA256SUMS`.

## AUR

After the `Release` workflow completes successfully for a pushed tag, the `Update AUR` workflow resolves the release tag from the completed workflow SHA, then renders and publishes:

- `synchrogit` from the GitHub source archive
- `synchrogit-bin` from the GitHub Release binary tarballs

The workflow needs the `AUR_SSH_PRIVATE_KEY` repository secret.

The `Update AUR` workflow can also be started by hand (workflow dispatch with a `tag` input) when the automatic chain did not run — for example after publishing release assets manually.

## Debian/RPM/APT/COPR

The Linux build jobs also produce `.deb` and `.rpm` packages (via `cargo-deb` and `cargo-generate-rpm`, metadata in `Cargo.toml`) and attach them to the GitHub Release. After a successful `Release` run:

- `Update APT repo` downloads the release `.deb`s, regenerates the flat signed repo, and pushes it to [`partanskiy/apt-repo`](https://github.com/partanskiy/apt-repo) (served via GitHub Pages). Needs the `APT_SSH_PRIVATE_KEY` (deploy key) and `APT_GPG_PRIVATE_KEY` (repo signing key) secrets.
- The **Update COPR** workflow requests one build after the `Release` workflow
  completes successfully. The [`partanskiy/synchrogit`](https://copr.fedorainfracloud.org/coprs/partanskiy/synchrogit/)
  COPR project uses a custom source method: its source script resolves the latest
  GitHub release, downloads its tarball, and renders
  `packaging/copr/synchrogit.spec.in` **from `main`**, so packaging fixes do not
  require a new tag. The workflow needs the `COPR_WEBHOOK_URL` Actions secret,
  containing the package-specific **custom** webhook URL. This URL authorizes
  builds; keep it out of source code and logs. The GitHub-specific COPR webhook
  endpoint is for SCM packages and does not rebuild this custom-source package.

Use **Update COPR** manually to rebuild the latest stable release when needed.
Automatic requests skip old releases, failed workflows and forks. Release asset
and notes edits do not trigger builds. Keep the old GitHub release-event webhook
disabled: it would start duplicate builds for these edits.

When configuring another repository, obtain the owner's approval before adding
its custom webhook URL to `COPR_WEBHOOK_URL`, and disable any direct release
webhook before enabling the workflow.

If a request times out, check COPR before rerunning: it may already have queued
the build. The workflow deliberately does not retry POST requests automatically.

The APT workflow supports the same manual `workflow_dispatch` fallback as the AUR and Homebrew publishers. Nix needs no publishing at all — the flake in the repo builds from the tagged source.

## Homebrew

The `Update Homebrew tap` workflow follows the same pattern: after a successful `Release` run it renders `packaging/brew/synchrogit.rb.in` with the macOS tarball URLs and checksums and pushes the formula to [`partanskiy/homebrew-tap`](https://github.com/partanskiy/homebrew-tap). It needs the `TAP_SSH_PRIVATE_KEY` repository secret (a deploy key with write access on the tap repo) and supports the same manual `workflow_dispatch` fallback.

Prerelease tags such as `v0.1.0-rc.1` build GitHub Release artifacts but are skipped by the AUR workflow.

## Windows and Android

Windows x86_64 is published as a ZIP containing the executable, example config,
optional MinGit installer, startup scripts and licenses. CI builds this archive
and runs the daemon/control tests on Windows with both Git backends.

Android uses the stable asset name `synchrogit-android.apk` for Obtainium and a
stable application ID `dev.synchrogit.app`. Debug builds use `.debug` and cannot
replace a release installation. Release signing requires the repository secrets
`ANDROID_KEYSTORE_BASE64` and `ANDROID_KEYSTORE_PASSWORD`, alias `synchrogit`.
Back up the signing key securely; replacing it breaks normal in-place updates.
The workflow removes the temporary keystore after signing and verifies the APK.

Android versionCode is derived from YY.M.PATCH as
`YY * 1000000 + M * 10000 + PATCH * 100 + 99`; keep PATCH below 100 so the ordering
remains monotonic. Prerelease sequencing requires an explicit versionCode policy
before publishing multiple APK prereleases of the same base version.

## Dependency notices

Update THIRD_PARTY_LICENSES.html when changing Cargo.lock:

```sh
cargo install cargo-about --version 0.9.2 --locked --features cli
python3 scripts/license-notices.py
```

The generated file accompanies the standalone archives, distribution packages
and APK. The generator includes the upstream native-library license files as
well as the Rust wrapper notices.
