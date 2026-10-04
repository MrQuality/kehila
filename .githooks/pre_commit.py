#!/usr/bin/env python3
"""Run the test suite against staged files in an isolated directory.

Git for Windows runs the extensionless launcher using its bundled sh. The bat
launcher is provided for direct cmd use; all policy and execution live in Python.
"""
from pathlib import Path
import argparse
import os
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def git(*args):
    return subprocess.check_output(["git", *args], cwd=ROOT, timeout=30)


def fail(code, message):
    print(f"VERIFICATION_FAILED: {message}. See docs/TESTING.md", file=sys.stderr)
    return code


def requires_tests(path):
    return (path.startswith(("src/", "go/", "packages/", "scripts/", ".githooks/", "tests/", ".github/workflows/"))
            and not path.endswith(".md")) or path in {
                "Cargo.toml", "Cargo.lock", "go.work", "go.work.sum", "package.json",
                "package-lock.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "docker-compose.yml",
                "docs/engineering/requirements.json", "docs/engineering/ENGINEERING-STANDARD.md",
                "docs/engineering/STANDARD-PREAMBLE.md"}


def requires_live(paths):
    manifests = {"Cargo.toml", "Cargo.lock", "go.work", "go.work.sum", "package.json",
                 "package-lock.json", "pnpm-lock.yaml", "pnpm-workspace.yaml", "docker-compose.yml"}
    return any(path.startswith(("src/io/", "go/io/", ".githooks/", "scripts/", "tests/", ".github/workflows/"))
               or Path(path).name in manifests or path.endswith(("go.mod", "go.sum")) for path in paths)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="CI: compare this ancestor with checked-out HEAD")
    args = parser.parse_args()
    if args.base:
        git("rev-parse", "--verify", args.base + "^{commit}")
        if git("diff", "--cached", "--name-only"):
            return fail(3, "CI --base requires the index to match HEAD")
        diff_args = [args.base, "HEAD"]
    else:
        diff_args = ["--cached"]
    changed = git("diff", *diff_args, "--name-only", "--no-renames", "-z").decode("utf-8").split("\0")
    changed = [path for path in changed if path]
    if not changed:
        print("No changes to verify")
        return 0
    if not any(requires_tests(path) for path in changed):
        print("Documentation-only change; no test run required")
        return 0
    tree_before = git("write-tree")
    with tempfile.TemporaryDirectory(prefix="yaja-index-") as temporary:
        snapshot = Path(temporary)
        git("checkout-index", "--all", "--force", "--prefix=" + snapshot.as_posix() + "/")
        # Reject symlinks/gitlinks in executable input, including Windows exports
        # where symlinks may otherwise look like ordinary text files.
        for entry in git("ls-files", "--stage", "-z").split(b"\0"):
            if entry and entry.split(b" ", 1)[0] not in (b"100644", b"100755"):
                return fail(3, "Symlinks and submodules are unsupported in the verified snapshot")
        live = requires_live(changed)
        command = [sys.executable, str(snapshot / "scripts/verify.py")]
        if not live:
            command.append("--pure")
        # Build cache is outside the snapshot; source still comes only from index.
        environment = os.environ.copy()
        environment["CARGO_TARGET_DIR"] = str(ROOT / "target")
        try:
            subprocess.run(command, cwd=snapshot, env=environment, check=True, timeout=600)
        except (OSError, subprocess.SubprocessError) as error:
            return fail(3, f"Staged verification failed: {error}")
        if git("write-tree") != tree_before:
            return fail(3, "Git index changed during verification; retry")
    print("Staged snapshot verified")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        sys.exit(fail(3, str(error)))
