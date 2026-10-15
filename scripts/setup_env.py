#!/usr/bin/env python3
"""Configure hooks, optionally start disposable infrastructure, and check readiness."""
from pathlib import Path
import argparse
import shutil
import stat
import subprocess
import sys
from verification_budget import DeadlineExceeded, command_timeout

ROOT = Path(__file__).resolve().parents[1]


def run(command, **kwargs):
    return subprocess.run(command, cwd=ROOT, check=True, timeout=600, **kwargs)


def compose_command(preferred="auto", environment=None):
    for engine, compose in (("podman", ["podman", "compose"]), ("docker", ["docker", "compose"])):
        if preferred != "auto" and engine != preferred:
            continue
        if not shutil.which(engine):
            continue
        try:
            subprocess.run([engine, "info"], env=environment, check=True, capture_output=True, timeout=command_timeout(15))
            subprocess.run(compose + ["version"], env=environment, check=True, capture_output=True, timeout=command_timeout(15))
            return compose
        except DeadlineExceeded:
            raise
        except (subprocess.SubprocessError, OSError):
            continue
    raise RuntimeError("Start Podman (podman machine start on Windows) and install a Compose provider, or start Docker.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--start", action="store_true", help="Start the four local containers")
    parser.add_argument("--hooks-only", action="store_true")
    parser.add_argument("--engine", choices=("auto", "podman", "docker"), default="auto")
    args = parser.parse_args()
    for executable in ("git", "cargo", "go"):
        if not shutil.which(executable):
            raise RuntimeError(f"Required tool missing: {executable}")
    run(["git", "config", "core.hooksPath", ".githooks"])
    hook = ROOT / ".githooks/pre-commit"
    hook.chmod(hook.stat().st_mode | stat.S_IXUSR | stat.S_IXGRP | stat.S_IXOTH)
    if args.hooks_only:
        return
    if args.start:
        run(compose_command(args.engine) + ["-f", "docker-compose.yml", "up", "-d"])
    run([sys.executable, "scripts/healthcheck.py", "--wait", "180"])
    run([sys.executable, "tests/integration/nats_probe.py"])


if __name__ == "__main__":
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f"SETUP_FAILED: {error}", file=sys.stderr)
        sys.exit(1)
