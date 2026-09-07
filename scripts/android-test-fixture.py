#!/usr/bin/env python3
"""Produce disposable, relocatable Git repos for Android instrumentation tests."""
from pathlib import Path
import subprocess
import tempfile
import zipfile

root = Path(__file__).resolve().parents[1]
def git(path, *args):
    subprocess.run(["git", "-C", str(path), *args], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
with tempfile.TemporaryDirectory() as directory:
    work = Path(directory)
    git(work, "init", "--bare", "--initial-branch=main", "remote.git")
    git(work, "clone", str(work / "remote.git"), "a")
    git(work / "a", "config", "user.name", "Android Test")
    git(work / "a", "config", "user.email", "test@example.com")
    (work / "a/note.md").write_text("initial\n")
    git(work / "a", "add", "note.md")
    git(work / "a", "commit", "-m", "initial")
    git(work / "a", "push", "-u", "origin", "main")
    git(work, "clone", str(work / "remote.git"), "b")
    for name in ["a", "b"]:
        git(work / name, "config", "user.name", "Android Test")
        git(work / name, "config", "user.email", "test@example.com")
        git(work / name, "config", "core.autocrlf", "false")
        git(work / name, "remote", "set-url", "origin", "SYNCHROGIT_TEST_REMOTE")
    target = root / "android/app/src/androidTest/assets/sync-fixture.zip"
    target.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(target, "w", zipfile.ZIP_DEFLATED) as archive:
        for path in sorted(work.rglob("*")):
            # libgit2's local transport needs empty objects/pack directories.
            archive.write(path, path.relative_to(work))
