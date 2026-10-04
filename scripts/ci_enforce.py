#!/usr/bin/env python3
"""Select the actual GitHub event base and exercise the same commit gate."""
from pathlib import Path
import json
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
event = json.loads(Path(os.environ["GITHUB_EVENT_PATH"]).read_text(encoding="utf-8"))
base = event.get("pull_request", {}).get("base", {}).get("sha") or event.get("before")
if base and set(base) != {"0"}:
    subprocess.run([sys.executable, ".githooks/pre_commit.py", "--base", base], cwd=ROOT, check=True)
else:
    # Root push: simulate the initial staged commit only inside a disposable clone.
    import tempfile
    with tempfile.TemporaryDirectory(prefix="kehila-ci-root-") as temporary:
        subprocess.run(["git", "clone", "--no-hardlinks", str(ROOT), temporary], check=True)
        # A SHA checkout is detached: deleting HEAD would invalidate the repository.
        # Point HEAD at a fresh unborn branch while preserving the entire index.
        branch = "refs/heads/kehila-ci-root"
        subprocess.run(["git", "update-ref", "-d", branch], cwd=temporary, check=True)
        subprocess.run(["git", "symbolic-ref", "HEAD", branch], cwd=temporary, check=True)
        subprocess.run([sys.executable, ".githooks/pre_commit.py"], cwd=temporary, check=True)
