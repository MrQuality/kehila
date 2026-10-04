#!/usr/bin/env python3
"""The shared local/CI verification matrix; missing tools and skipped I/O fail closed."""
from pathlib import Path
import argparse
import os
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pure", action="store_true", help="Run unit tests without external services")
    args = parser.parse_args()
    commands = [[sys.executable, "-m", "unittest", "discover", "-s", "tests", "-v"],
                [sys.executable, "scripts/check_branding.py"],
                [sys.executable, "scripts/check_engineering.py"],
                [sys.executable, "scripts/go_static.py"]]
    if not args.pure:
        commands += [[sys.executable, "scripts/healthcheck.py"], [sys.executable, "tests/integration/nats_probe.py"]]
    commands += [["cargo", "test", "--locked", "-p", "yaja_query", "-p", "task_contract"] if args.pure
                 else ["cargo", "test", "--locked", "--workspace"],
                 [sys.executable, "scripts/go_test.py"]]
    if not args.pure:
        commands.append([sys.executable, "tests/integration/task_path.py"])
    for command in commands:
        print("VERIFY:", " ".join(command), flush=True)
        subprocess.run(command, cwd=ROOT, check=True, timeout=300)


if __name__ == "__main__":
    try:
        main()
    except (OSError, subprocess.SubprocessError) as error:
        print(f"ERR_TEST_EXECUTION_FAILED: {error}", file=sys.stderr)
        sys.exit(3)
