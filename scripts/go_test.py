#!/usr/bin/env python3
"""Run real Go tests; retry only temporary-directory cleanup on Windows locks."""
from pathlib import Path
import os
import shutil
import subprocess
import sys
import tempfile
import time

ROOT = Path(__file__).resolve().parents[1]
GO_PACKAGES = ["./go/pure/sync_contract/...", "./go/io/task_api/..."]


def main():
    temporary_root = Path(tempfile.gettempdir()).resolve()
    work = Path(tempfile.mkdtemp(prefix="yaja-go-", dir=temporary_root)).resolve()
    if work.parent != temporary_root or not work.name.startswith("yaja-go-"):
        raise RuntimeError("Refusing cleanup outside the allocated temporary directory")
    environment = os.environ.copy()
    environment["GOTMPDIR"] = str(work)
    try:
        # -work delegates deletion to us; it does not skip compilation or tests.
        command = ["go", "test", "-count=1", "-work", *(sys.argv[1:] or GO_PACKAGES)]
        result = subprocess.run(command, cwd=ROOT, env=environment, timeout=300, check=False)
        return result.returncode
    finally:
        deadline = time.monotonic() + 30
        while work.exists():
            try:
                shutil.rmtree(work)
            except PermissionError:
                if time.monotonic() >= deadline:
                    raise
                time.sleep(0.25)


if __name__ == "__main__":
    sys.exit(main())
