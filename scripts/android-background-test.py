#!/usr/bin/env python3
"""Exercise specialUse synchronization, recovery and scheduled fallback.

System timeout and clock overrides affect the whole device, so the full test
requires an emulator. It only writes the debug app's local Git fixture.
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
    matches = []
    def ready():
        nonlocal matches
        matches = [n for n in nodes() if n.get("text") == text and n.get("enabled") == "true"]
        assert len(matches) <= 1, f"Expected at most one {text} control"
        return bool(matches)
    wait_for(f"Control ready: {text}", ready, seconds=30)
    x1, y1, x2, y2 = map(int, re.findall(r"\d+", matches[0].get("bounds")))
    shell("input", "tap", str((x1 + x2) // 2), str((y1 + y2) // 2))


def open_app():
    shell("am", "start", "-W", "-n", APP + "/dev.synchrogit.app.MainActivity")


def reboot_emulator():
    assert emulator, "Never reboot a physical device in this test"
    previous_boot = shell("cat", "/proc/sys/kernel/random/boot_id")
    subprocess.run(["adb", "reboot"], check=True, timeout=30)
    subprocess.run(["adb", "wait-for-device"], check=True, timeout=60)
    wait_for("Emulator completes a real reboot",
             lambda: shell("getprop", "sys.boot_completed", check=False) == "1"
             and shell("cat", "/proc/sys/kernel/random/boot_id") != previous_boot,
             seconds=180)
    shell("input", "keyevent", "KEYCODE_WAKEUP")
    shell("wm", "dismiss-keyguard")


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
emulator = shell("getprop", "ro.kernel.qemu") == "1"
assert args.process_only or emulator, "Timeout tests require a disposable emulator"
assert int(shell("getprop", "ro.build.version.sdk")) >= 35
old_timeout = shell("device_config", "get", "activity_manager", TIMEOUT)
clock_restore = None
try:
    if not args.process_only:
        shell("device_config", "put", "activity_manager", TIMEOUT, "600000")
    if emulator:
        shell("input", "keyevent", "KEYCODE_WAKEUP")
        shell("wm", "dismiss-keyguard")
    setup = shell("am", "instrument", "-w", "-r", "-e", "class",
                  "dev.synchrogit.app.BackgroundLifecycleTest#prepareAdbScenario",
                  "-e", "background-scenario", "true",
                  APP + ".test/androidx.test.runner.AndroidJUnitRunner")
    assert "OK (1 test)" in setup, setup
    open_app()
    tap("Start")
    wait_for("Continuous service starts", foreground)
    service = shell("dumpsys", "activity", "services", APP)
    assert re.search(r"\bisForeground=true[^\n]*\btypes=0x40000000\b", service), \
        "Continuous sync must run as specialUse, without the dataSync type"
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
    wait_for("Service starts with a shortened dataSync limit", foreground)
    previous_interruption = preferences().get("interrupted_at")
    shell("input", "keyevent", "KEYCODE_HOME")
    # Stay in the background beyond the dataSync deadline and its grace period.
    # Poll throughout the window so a stop/restart cannot masquerade as survival.
    pid = shell("pidof", APP)
    deadline = time.monotonic() + 45
    while time.monotonic() < deadline:
        assert foreground(), "specialUse must outlast the dataSync limit"
        assert shell("pidof", APP) == pid, "The service must survive without a process restart"
        assert preferences().get("interrupted_at") == previous_interruption
        time.sleep(1)
    before = edit("beyond-datasync-deadline")
    wait_for("The watcher still synchronizes beyond the dataSync deadline", lambda: converged(before))
    results.append({"check": "specialUse outlasts the dataSync deadline and continues filesystem synchronization", "passed": True})

    # Stop just the service as its own UID, retaining the user's saved intent
    # and WorkManager jobs. A package force-stop would also suppress those jobs.
    # Android's stopservice command returns nonzero even when it stops the service.
    shell("run-as", APP, "am", "stopservice", "--user", "0", "-n", APP + "/dev.synchrogit.app.SyncService", check=False)
    wait_for("External service stop leaves continuous sync paused", lambda: not foreground())
    state = preferences()
    assert state["continuous"] == "true" and state["interruption"] == "service_stopped", state
    wait_for("Scheduled fallback remains registered after the service stops", lambda: bool(jobs()))
    before = edit("after-service-stop")
    # JobScheduler's -f bypasses platform constraints, but WorkManager also
    # checks wall-clock time against its minimum periodic interval. Advance
    # only this disposable emulator, after checking specialUse with real time.
    clock_restore = (int(shell("date", "+%s")) * 1000, time.monotonic(),
                     shell("settings", "get", "global", "auto_time"))
    shell("settings", "put", "global", "auto_time", "0")
    shell("cmd", "alarm", "set-time", str(clock_restore[0] + 16 * 60 * 1000))
    assert int(shell("date", "+%s")) * 1000 >= clock_restore[0] + 15 * 60 * 1000
    job_id, namespace = jobs()[0]
    command = ["cmd", "jobscheduler", "run", "-f"]
    if namespace and namespace != "null":
        command += ["-n", namespace]
    output = shell(*command, APP, job_id)
    assert "Running job" in output, output
    wait_for("Scheduled fallback synchronizes after the service stops", lambda: converged(before))
    assert not foreground(), "Fallback must perform one Git cycle without starting a foreground service"
    results.append({"check": "external service stop preserves working scheduled fallback", "passed": True})

    open_app()
    wait_for("Opening the app resumes requested continuous synchronization", foreground)
    assert preferences()["interruption"] == "service_stopped", "Preserve the last interruption for diagnosis"
    assert not any("|" + APP + "|" in line for line in shell("cmd", "notification", "list").splitlines())

    shell("input", "keyevent", "KEYCODE_HOME")
    reboot_emulator()
    wait_for("Boot restores requested continuous sync without opening the app", foreground)
    assert preferences()["continuous"] == "true"
    before = edit("after-device-reboot")
    wait_for("The watcher synchronizes after reboot without opening the app", lambda: converged(before))
    results.append({"check": "device reboot restores continuous service and filesystem synchronization", "passed": True})

    open_app()
    tap("Stop")
    wait_for("User Stop cancels automatic fallback", lambda: not foreground() and not jobs())
    assert preferences()["continuous"] == "false"
    shell("input", "keyevent", "KEYCODE_HOME")
    reboot_emulator()
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        assert not foreground(), "Boot must respect an explicit Stop"
        assert preferences()["continuous"] == "false"
        time.sleep(1)
    assert not jobs(), "Boot must not restore automatic fallback after explicit Stop"
    results.append({"check": "explicit Stop remains effective after device reboot", "passed": True})
    open_app()
    assert not foreground(), "Opening the app must respect an explicit Stop"
    results.append({"check": "resume respects user intent and keeps drawer notifications hidden", "passed": True})
    print(json.dumps(results, indent=2), flush=True)
except Exception:
    # The emulator contains only disposable fixtures, never user repositories.
    if emulator:
        directory = Path("android/app/build/reports/background-failure")
        directory.mkdir(parents=True, exist_ok=True)
        screenshot = subprocess.run(["adb", "exec-out", "screencap", "-p"],
                                    capture_output=True, timeout=30)
        (directory / "screen.png").write_bytes(screenshot.stdout)
        (directory / "service.txt").write_text(shell("dumpsys", "activity", "services", APP))
        (directory / "jobs.txt").write_text(shell("dumpsys", "jobscheduler", APP))
    raise
finally:
    if clock_restore is not None:
        original, elapsed, automatic = clock_restore
        shell("cmd", "alarm", "set-time", str(original + int((time.monotonic() - elapsed) * 1000)))
        if automatic == "null":
            shell("settings", "delete", "global", "auto_time")
        else:
            shell("settings", "put", "global", "auto_time", automatic)
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
