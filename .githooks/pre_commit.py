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

# Hook imports must not create artifacts in the checkout.
sys.dont_write_bytecode = True

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "scripts"))
from temp_work import owned_directory
from cleanup_errors import print_failure
from verification_settings import load as verification_settings


def git(*args, environment=None, input_bytes=None):
    return subprocess.check_output(["git", *args], cwd=ROOT, timeout=30,
                                   env=environment, input=input_bytes)


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


def verification_scope(paths):
    """Share conservative selection with CI; unknown inputs never skip tests."""
    root_docs = {"README.md", "CONTRIBUTING.md", "SECURITY.md", "CODE_OF_CONDUCT.md",
                 ".github/pull_request_template.md"}

    def ordinary_doc(path):
        return (path in root_docs or (path.startswith("docs/") and path.endswith(".md")))

    if not paths:
        return "full"
    if all(ordinary_doc(path) and not requires_tests(path) for path in paths):
        return "docs"
    if requires_live(paths) or any(not requires_tests(path) and not ordinary_doc(path)
                                   for path in paths):
        return "full"
    return "pure"


def head_commit():
    try:
        return git("rev-parse", "--verify", "--quiet", "HEAD^{commit}").strip()
    except subprocess.CalledProcessError as error:
        if error.returncode != 1:
            raise
        return None  # A genuinely unborn repository has no baseline commit.


def supported_tree(tree):
    entries = git("ls-tree", "-r", "-z", tree.decode("ascii")).split(b"\0")
    return all(not entry or entry.split(b" ", 1)[0] in (b"100644", b"100755")
               for entry in entries)


def export_source(tree, destination, index):
    """Export an immutable tree using metadata outside the source payload."""
    destination, index = Path(destination), Path(index)
    if (not destination.is_absolute() or not index.is_absolute()
            or index.exists() or destination == index or destination in index.parents):
        raise ValueError("Expected a new external snapshot index")
    destination.mkdir()
    environment = dict(os.environ, GIT_INDEX_FILE=str(index))
    git("read-tree", tree.decode("ascii"), environment=environment)
    git("checkout-index", "--all", "--force", "--prefix=" + destination.as_posix() + "/",
        environment=environment)


def verify_index_and_head(tree, head):
    if git("write-tree").strip() != tree or head_commit() != head:
        return fail(3, "Git index or HEAD changed during verification; retry")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base", help="CI: compare this ancestor with checked-out HEAD")
    parser.add_argument('--qualification', action='store_true',
                        help='Run full verification of the staged tree, including an empty/prose-only diff')
    args = parser.parse_args()
    head_before = head_commit()
    tree_before = git("write-tree").strip()
    if args.base:
        base = git("rev-parse", "--verify", args.base + "^{commit}").strip()
        if head_before is None or tree_before != git("rev-parse", head_before.decode("ascii") + "^{tree}").strip():
            return fail(3, "CI --base requires the index to match HEAD")
    else:
        base = head_before if head_before is not None else git("mktree", input_bytes=b"").strip()
    changed = git("diff", base.decode("ascii"), tree_before.decode("ascii"),
                  "--name-only", "--no-renames", "-z").decode("utf-8").split("\0")
    changed = [path for path in changed if path]
    # Both policy selection and supported-mode validation describe this tree,
    # rather than consulting a possibly changing real index a second time.
    if not supported_tree(tree_before):
        return fail(3, "Symlinks and submodules are unsupported in the verified snapshot")
    if not changed and not args.qualification:
        print("No changes to verify")
        return verify_index_and_head(tree_before, head_before)
    scope = 'full' if args.qualification else verification_scope(changed)
    if scope == "docs":
        print("Documentation-only change; no test run required")
        return verify_index_and_head(tree_before, head_before)
    # Validate against the original checkout, before source moves to a snapshot.
    settings = verification_settings()
    with owned_directory("kehila-index-") as temporary:
        snapshot = Path(temporary) / "source"
        export_source(tree_before, snapshot, Path(temporary) / "index")
        print("Verification source tree:", tree_before.decode("ascii"), flush=True)
        verifier = "container_verify.py" if os.name == "nt" else "verify.py"
        allowance = settings['gate_seconds'] if os.name == 'nt' else 600
        command = [sys.executable, str(snapshot / "scripts" / verifier), "--timeout", str(allowance)]
        if not args.qualification and scope == 'full':
            command.append('--development')
        elif not args.qualification:
            command.append("--pure")
        print('Verification mode:', 'qualification' if args.qualification else
              'development' if scope == 'full' else 'pure', flush=True)
        environment = os.environ.copy()
        environment.setdefault("CARGO_TARGET_DIR", str(ROOT / "target"))
        # Keep verification failure active while owned cleanup runs.
        subprocess.run(command, cwd=snapshot, env=environment, check=True)
        status = verify_index_and_head(tree_before, head_before)
        if status:
            return status
    print("Staged snapshot verified")
    return 0


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print_failure("VERIFICATION_FAILED", error)
        sys.exit(3)
