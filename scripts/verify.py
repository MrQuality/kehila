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
    modes = parser.add_mutually_exclusive_group()
    modes.add_argument("--pure", action="store_true", help="Run unit tests without external services")
    modes.add_argument("--docs", action="store_true", help="Run Python, naming and engineering metadata checks")
    args = parser.parse_args()
    commands = [[sys.executable, "-m", "unittest", "discover", "-s", "tests", "-v"],
                [sys.executable, "scripts/check_branding.py"],
                [sys.executable, "scripts/check_engineering.py"]]
    if not args.docs:
        commands.append([sys.executable, "scripts/go_static.py"])
    if not args.pure and not args.docs:
        commands += [[sys.executable, "scripts/healthcheck.py"], [sys.executable, "tests/integration/nats_probe.py"]]
    if not args.docs:
        commands += [["cargo", "test", "--locked", "-p", "kehila_query", "-p", "task_contract"] if args.pure
                     else ["cargo", "test", "--locked", "--workspace"],
                     [sys.executable, "scripts/go_test.py"]]
    if not args.pure and not args.docs:
        commands.append([sys.executable, "tests/integration/task_path.py"])
    for command in commands:
        print("VERIFY:", " ".join(command), flush=True)
        # Observed Windows workspace runs exceed 300s despite passing tests.
        # Keep a finite command budget inside the staged hook's 600s deadline.
        timeout = 450 if os.name == "nt" and command[0] == "cargo" else 300
        subprocess.run(command, cwd=ROOT, check=True, timeout=timeout)


if __name__ == "__main__":
    try:
        main()
    except (OSError, subprocess.SubprocessError) as error:
        print(f"ERR_TEST_EXECUTION_FAILED: {error}", file=sys.stderr)
        sys.exit(3)
