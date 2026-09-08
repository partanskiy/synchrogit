#!/usr/bin/env python3
"""Trigger COPR after a successful release workflow, not each release edit."""
import json
import os
from pathlib import Path
import re
import subprocess
import sys
import urllib.error
import urllib.parse
import urllib.request


def should_publish(event_name, event, repository, release, tag_commit):
    tag = release.get("tag_name", "")
    if release.get("draft") or release.get("prerelease") or not re.fullmatch(r"v\d+\.\d+\.\d+", tag):
        return False
    if event_name == "workflow_dispatch":
        return True
    run = event.get("workflow_run", {})
    return (
        event_name == "workflow_run"
        and run.get("conclusion") == "success"
        and run.get("event") == "push"
        and run.get("head_repository", {}).get("full_name") == repository
        and run.get("head_branch") == tag
        and run.get("head_sha") == tag_commit
    )


def request_build(url, tag, opener=urllib.request.urlopen):
    parsed = urllib.parse.urlsplit(url)
    if (parsed.scheme != "https" or parsed.hostname != "copr.fedorainfracloud.org"
            or parsed.port not in (None, 443) or parsed.username or parsed.password
            or not parsed.path.startswith("/webhooks/custom/") or parsed.query or parsed.fragment):
        raise ValueError("Configure COPR_WEBHOOK_URL with this package's HTTPS custom webhook URL")
    request = urllib.request.Request(url, method="POST", data=json.dumps({"release": tag}).encode(),
                                     headers={"Content-Type": "application/json"})
    try:
        # A timed-out POST may already have queued a build. Do not retry it
        # automatically: inspect COPR before manually rerunning the workflow.
        with opener(request, timeout=30) as response:
            response.read()
    except (urllib.error.URLError, TimeoutError) as error:
        # Exception strings can contain the secret webhook URL.
        raise RuntimeError("COPR request failed; inspect COPR before retrying because the build may already be queued") from None


def main():
    repository = os.environ["GITHUB_REPOSITORY"]
    release = json.loads(subprocess.check_output(["gh", "api", f"repos/{repository}/releases/latest"]))
    tag = release["tag_name"]
    event_name = os.environ["GITHUB_EVENT_NAME"]
    event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text())
    refs = subprocess.check_output(["git", "ls-remote", "--tags", f"https://github.com/{repository}.git",
                                    f"refs/tags/{tag}", f"refs/tags/{tag}^{{}}"], text=True)
    commits = {ref: sha for sha, ref in (line.split() for line in refs.splitlines())}
    commit = commits.get(f"refs/tags/{tag}^{{}}", commits.get(f"refs/tags/{tag}"))
    if not commit:
        raise RuntimeError("The published release tag could not be resolved")
    if not should_publish(event_name, event, repository, release, commit):
        print("Skipping: this event is not the completed workflow for the latest stable release")
        return
    request_build(os.environ.get("COPR_WEBHOOK_URL", ""), tag)
    print(f"Requested one COPR build for {tag}")


if __name__ == "__main__":
    try:
        main()
    except (RuntimeError, ValueError) as error:
        print(str(error), file=sys.stderr)
        sys.exit(1)
