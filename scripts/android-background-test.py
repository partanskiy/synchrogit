#!/usr/bin/env python3
"""Exercise real Android process recovery and timeouts on a disposable emulator.

The system timeout override affects every dataSync service, so this script
refuses physical devices. It only writes the debug app's local Git fixture.
"""
import argparse
import json
from pathlib import Path
import re
import shlex
import subprocess
import time
import xml.etree.ElementTree as ET

APP = "dev.synchrogit.app.debug"
ROOT = "files/background-lifecycle-fixture"
UI = "/data/local/tmp/synchrogit-background-ui.xml"
TIMEOUT = "data_sync_fgs_timeout_duration"
results = []


def shell(*args, data=None, check=True):
    result = subprocess.run(["adb", "shell", "-T", shlex.join(args)], input=data,
                            capture_output=True, text=True, timeout=30)
    if check and result.returncode:
        raise RuntimeError(f"adb command failed: {args[:3]}: {result.stderr}")
    return result.stdout.strip()


def wait_for(description, predicate, seconds=90):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if predicate():
            print(description, flush=True)
            return
        time.sleep(1)
    raise AssertionError(description)


def foreground():
    return "isForeground=true" in shell("dumpsys", "activity", "services", APP)


def preferences():
    raw = shell("run-as", APP, "cat", "shared_prefs/synchrogit-background.xml")
    return {e.get("name"): e.get("value", e.text) for e in ET.fromstring(raw)}


def nodes():
    shell("uiautomator", "dump", UI)
    raw = shell("cat", UI)
    return [n for n in ET.fromstring(raw).iter("node") if n.get("package") == APP]


def tap(text):
    matches = [n for n in nodes() if n.get("text") == text and n.get("enabled") == "true"]
    assert len(matches) == 1, f"Expected one {text} control"
    x1, y1, x2, y2 = map(int, re.findall(r"\d+", matches[0].get("bounds")))
    shell("input", "tap", str((x1 + x2) // 2), str((y1 + y2) // 2))


def open_app():
    shell("am", "start", "-W", "-n", APP + "/dev.synchrogit.app.MainActivity")


def ref(repository):
    git = ".git/" if repository == "a" else ""
    return shell("run-as", APP, "cat", f"{ROOT}/{repository}/{git}refs/heads/main")


def edit(name):
    before = ref("remote.git")
    command = "cat > " + shlex.quote(f"{ROOT}/a/{name}.md")
    shell("run-as", APP, "sh", "-c", command, data=f"{name} {time.time_ns()}\n")
    return before


def converged(before):
    remote = ref("remote.git")
    return remote != before and remote == ref("a")


def jobs():
    dump = shell("dumpsys", "jobscheduler")
    found = []
    # Namespaced jobs use "JOB namespace:uid/id", without the usual "#".
    for block in re.split(r"(?=^[ \t]*JOB )", dump, flags=re.M):
        first = block.lstrip().splitlines()[0] if block.strip() else ""
        if APP + "/androidx.work.impl.background.systemjob.SystemJobService" not in first:
            continue
        match = re.search(r"/(\d+):", first)
        assert match, "Unrecognized WorkManager job identifier: " + first
        namespace = re.match(r"JOB ([^:#\s]+):", first)
        found.append((match.group(1), namespace.group(1) if namespace else None))
    return found


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--process-only", action="store_true", help="Test only debug-process recovery; never change the system timeout")
args = parser.parse_args()
assert args.process_only or shell("getprop", "ro.kernel.qemu") == "1", "Timeout tests require a disposable emulator"
assert int(shell("getprop", "ro.build.version.sdk")) >= 35
old_timeout = shell("device_config", "get", "activity_manager", TIMEOUT)
try:
    if not args.process_only:
        shell("device_config", "put", "activity_manager", TIMEOUT, "600000")
    setup = shell("am", "instrument", "-w", "-r", "-e", "class",
                  "dev.synchrogit.app.BackgroundLifecycleTest#prepareAdbScenario",
                  "-e", "background-scenario", "true",
                  APP + ".test/androidx.test.runner.AndroidJUnitRunner")
    assert "OK (1 test)" in setup, setup
    open_app()
    tap("Start")
    wait_for("Continuous service starts", foreground)
    shell("input", "keyevent", "KEYCODE_HOME")
    before = edit("before-process-death")
    wait_for("Initial watcher pushes to the local remote", lambda: converged(before))

    pid = shell("pidof", APP)
    assert pid.isdigit()
    shell("run-as", APP, "kill", "-9", pid)
    wait_for("Android restores the sticky service after process death",
             lambda: shell("pidof", APP, check=False) not in ("", pid) and foreground())
    before = edit("after-process-death")
    wait_for("Restored Rust watcher pushes without reopening the app", lambda: converged(before))
    results.append({"check": "process death restores service and filesystem synchronization", "passed": True})

    open_app()
    tap("Stop")
    wait_for("Explicit Stop removes the service", lambda: not foreground())
    wait_for("Explicit Stop cancels automatic scheduled work", lambda: not jobs())
    if args.process_only:
        assert preferences()["continuous"] == "false"
        print(json.dumps(results, indent=2), flush=True)
        raise SystemExit(0)
    shell("device_config", "put", "activity_manager", TIMEOUT, "15000")
    tap("Start")
    wait_for("Service starts for the Android timeout test", foreground)
    shell("input", "keyevent", "KEYCODE_HOME")
    wait_for("Android timeout stops the service gracefully", lambda: not foreground())
    state = preferences()
    assert state["continuous"] == "true" and state["interruption"] == "timeout", state
    wait_for("Scheduled fallback remains registered after the timeout", lambda: bool(jobs()))
    before = edit("after-android-timeout")
    job_id, namespace = jobs()[0]
    command = ["cmd", "jobscheduler", "run", "-f"]
    if namespace and namespace != "null":
        command += ["-n", namespace]
    output = shell(*command, APP, job_id)
    assert "Running job" in output, output
    wait_for("Scheduled fallback synchronizes after the dataSync budget expires", lambda: converged(before))
    assert not foreground(), "Fallback must not restart a time-limited foreground service"
    results.append({"check": "real Android timeout preserves working scheduled fallback", "passed": True})

    open_app()
    wait_for("Opening the app resumes requested continuous synchronization", foreground)
    assert preferences()["interruption"] == "timeout", "Preserve the last interruption for diagnosis"
    assert not any("|" + APP + "|" in line for line in shell("cmd", "notification", "list").splitlines())
    tap("Stop")
    wait_for("User Stop cancels automatic fallback", lambda: not foreground() and not jobs())
    assert preferences()["continuous"] == "false"
    open_app()
    assert not foreground(), "Opening the app must respect an explicit Stop"
    results.append({"check": "resume respects user intent and keeps drawer notifications hidden", "passed": True})
    print(json.dumps(results, indent=2), flush=True)
finally:
    if not args.process_only:
        if old_timeout == "null":
            shell("device_config", "delete", "activity_manager", TIMEOUT)
        else:
            shell("device_config", "put", "activity_manager", TIMEOUT, old_timeout)
    shell("am", "force-stop", APP)
    shell("rm", "-f", UI)
    out = Path("android/app/build/reports/background-lifecycle.json")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(json.dumps(results, indent=2) + "\n")
