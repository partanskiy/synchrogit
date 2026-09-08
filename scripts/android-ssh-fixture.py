#!/usr/bin/env python3
"""Loopback-only disposable SSH Git server for Android instrumentation.

Only a fresh test key is authorized, only two Git commands can run, and only
against this fixture's bare repository. No user SSH keys/config are read.
"""
import argparse
import getpass
import json
import os
from pathlib import Path
import shlex
import shutil
import socket
import subprocess
import sys
import time


def serve(repository):
    command = shlex.split(os.environ.get("SSH_ORIGINAL_COMMAND", ""))
    if len(command) != 2 or command[0] not in ("git-upload-pack", "git-receive-pack") or command[1] != repository:
        sys.exit("Only the disposable Git repository is available")
    os.execv(shutil.which(command[0]), command)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--serve")
    parser.add_argument("--directory", default="target/android-ssh-fixture")
    parser.add_argument("--port", type=int, default=22229)
    parser.add_argument("--stop", action="store_true")
    args = parser.parse_args()
    if args.serve:
        serve(args.serve)
    directory = Path(args.directory).resolve()
    if args.stop:
        if not (directory / "server.pid").exists():
            return
        pid = int((directory / "server.pid").read_text())
        # Avoid killing an unrelated process if a stale PID has been reused.
        process = Path(f"/proc/{pid}/cmdline")
        if process.exists() and str(directory / "sshd_config").encode() in process.read_bytes():
            os.kill(pid, 15)
        print("Stopped the disposable SSH fixture")
        return
    directory.mkdir(mode=0o700, parents=True, exist_ok=False)
    def run(*command):
        return subprocess.run(command, check=True, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True).stdout
    for name in ("host_key", "client_key"):
        run("ssh-keygen", "-q", "-t", "ed25519", "-N", "", "-f", str(directory / name))
    (directory / "authorized_keys").write_text("restrict " + (directory / "client_key.pub").read_text())
    (directory / "authorized_keys").chmod(0o600)
    repository = str(directory / "remote.git")
    run("git", "init", "--bare", "--initial-branch=main", repository)
    seed = directory / "seed"
    run("git", "clone", repository, str(seed))
    run("git", "-C", str(seed), "config", "user.name", "SSH Fixture")
    run("git", "-C", str(seed), "config", "user.email", "fixture@example.com")
    (seed / "note.md").write_text("SSH fixture\n")
    run("git", "-C", str(seed), "add", "note.md")
    run("git", "-C", str(seed), "commit", "-m", "initial")
    run("git", "-C", str(seed), "push", "origin", "main")
    forced = shlex.join([sys.executable, str(Path(__file__).resolve()), "--serve", repository])
    config = directory / "sshd_config"
    config.write_text(f"""Port {args.port}
ListenAddress 127.0.0.1
HostKey {directory / 'host_key'}
PidFile {directory / 'sshd.pid'}
AuthorizedKeysFile {directory / 'authorized_keys'}
AllowUsers {getpass.getuser()}
AuthenticationMethods publickey
PubkeyAuthentication yes
PasswordAuthentication no
KbdInteractiveAuthentication no
UsePAM yes
DisableForwarding yes
PermitTTY no
ForceCommand {forced}
LogLevel ERROR
""")
    sshd = shutil.which("sshd") or "/usr/sbin/sshd"
    run(sshd, "-t", "-f", str(config))
    with (directory / "server.log").open("w") as log:
        process = subprocess.Popen([sshd, "-D", "-e", "-f", str(config)], stdout=log, stderr=log, start_new_session=True)
    (directory / "server.pid").write_text(str(process.pid))
    for _ in range(50):
        if process.poll() is not None:
            sys.exit((directory / "server.log").read_text())
        try:
            with socket.create_connection(("127.0.0.1", args.port), timeout=0.2):
                break
        except OSError:
            time.sleep(0.1)
    else:
        process.terminate()
        sys.exit("SSH fixture did not start")
    fixture = {
        "url": f"ssh://{getpass.getuser()}@127.0.0.1:{args.port}{repository}",
        "private_key": (directory / "client_key").read_text(),
        "host_fingerprint": run("ssh-keygen", "-lf", str(directory / "host_key.pub"), "-E", "sha256").split()[1],
    }
    destination = Path(__file__).resolve().parents[1] / "android/app/src/androidTest/assets/ssh-fixture.json"
    destination.parent.mkdir(parents=True, exist_ok=True)
    destination.write_text(json.dumps(fixture))
    destination.chmod(0o600)
    print(f"Disposable SSH Git fixture listening on 127.0.0.1:{args.port}; connect Android using adb reverse tcp:{args.port} tcp:{args.port}")


if __name__ == "__main__":
    main()
