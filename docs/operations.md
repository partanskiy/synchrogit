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

## Deployment models (Linux)

The packages ship two systemd units; pick one per machine.

### 1. User service (desktops)

Runs inside your user session, starts at login:

```sh
systemctl --user enable --now synchrogit.service
systemctl --user status synchrogit.service
journalctl --user -u synchrogit.service -f
```

### 2. User service + lingering (recommended for headless and always-on machines)

Same unit, but the user manager — and the daemon with it — starts at boot and survives logout:

```sh
sudo loginctl enable-linger $USER
systemctl --user enable --now synchrogit.service
```

Check lingering with `loginctl user-status $USER` (`Linger: yes`). Root can inspect another user's daemon with `systemctl --user -M <user>@ status synchrogit`.

### 3. System template (root-managed fleets)

`synchrogit@<user>.service` runs the daemon for one user under the system manager:

```sh
sudo systemctl enable --now synchrogit@alice.service
sudo systemctl status synchrogit@alice.service
```

The template binds the control socket at `/run/synchrogit/<user>/synchrogit.sock`; the CLI probes that location automatically, so `synchrogit status` works from the user's shell without extra flags. Config resolution is unchanged — the daemon runs as the instance user and reads their `~/.config/synchrogit/config.toml`, falling back to the machine-wide `/etc/synchrogit/config.toml`. The lookup is first-match, not a merge: a user config completely shadows the `/etc` one.

For manual installs, copy `packaging/systemd/synchrogit.service` to `~/.config/systemd/user/synchrogit.service` (or `packaging/systemd/synchrogit@.service` to `/etc/systemd/system/`) and edit `ExecStart` if the binary lives elsewhere.

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
