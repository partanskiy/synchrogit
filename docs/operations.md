# Operations

## Run

```sh
synchrogit run
```

The daemon opens a Unix socket at `$XDG_RUNTIME_DIR/synchrogit.sock`. If `XDG_RUNTIME_DIR` is unset, it falls back to `/tmp/synchrogit-$UID.sock`.

Use `--config` and `--socket` for explicit paths:

```sh
synchrogit run --config ~/.config/synchrogit/config.toml --socket /tmp/synchrogit.sock
```

## Control

```sh
synchrogit status
synchrogit sync
synchrogit sync notes
synchrogit reload
```

- `status`: prints one tab-separated line per repo: name, path, branch, upstream, running flag, last outcome, failure count, and last error.
- `sync`: queues an immediate sync cycle for all repos.
- `sync <repo>`: queues one repo by name.
- `reload`: asks the daemon to reload the config now.

The daemon also watches the config file parent directory and reloads after edits or atomic replacement. Malformed config is rejected and the previous worker set keeps running.

## Systemd User Service

The packaged service assumes `/usr/bin/synchrogit`:

```sh
systemctl --user enable --now synchrogit.service
systemctl --user status synchrogit.service
journalctl --user -u synchrogit.service -f
```

For manual installs, copy `packaging/synchrogit.service` to `~/.config/systemd/user/synchrogit.service` and edit `ExecStart` if the binary lives elsewhere.

## Release

Releases are tag-driven:

```sh
git fetch origin
git switch main
git pull --ff-only origin main
git tag -a vX.Y.Z -m "vX.Y.Z"
git push origin vX.Y.Z
```

The `Release` workflow checks that the tag matches `Cargo.toml`, builds flat Linux tarballs, publishes checksums, and creates the GitHub Release. After that workflow completes successfully for a stable tag, `Update AUR` resolves the tag from the completed workflow SHA and publishes the `synchrogit` and `synchrogit-bin` AUR packages.
