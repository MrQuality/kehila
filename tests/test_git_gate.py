"""Verify staged test execution and hook dispatch in temporary Git repositories."""
from pathlib import Path
import os
import stat
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


class GitGateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="kehila-gate-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.environment = os.environ.copy()
        # Hook-inherited Git variables must not redirect commands to the parent repo.
        for key in list(self.environment):
            if key.startswith("GIT_"):
                self.environment.pop(key)
        self.environment.update({"GIT_AUTHOR_NAME": "Policy Test", "GIT_COMMITTER_NAME": "Policy Test",
                                 "GIT_AUTHOR_EMAIL": "test@example.invalid", "GIT_COMMITTER_EMAIL": "test@example.invalid"})
        self.git("init", "-b", "main")
        self.git("config", "core.autocrlf", "false")
        for relative in (".githooks/pre_commit.py", ".githooks/pre-commit"):
            self.write(relative, (ROOT / relative).read_text(encoding="utf-8"))
        hook = self.root / ".githooks/pre-commit"
        hook.chmod(hook.stat().st_mode | stat.S_IXUSR)
        self.write("src/pure/value.py", "assert 2 + 2 == 4\n")
        self.write("docs/spec.md", "# Fixture contract\n\nArithmetic is pure.\n")
        self.write("scripts/verify.py", "import subprocess, sys\nassert '--pure' in sys.argv\nsubprocess.run([sys.executable, 'src/pure/value.py'], check=True)\n")
        self.git("add", "--all")
        self.git("update-index", "--chmod=+x", ".githooks/pre-commit")
        tree = self.git("write-tree").stdout.strip()
        commit = self.git("commit-tree", tree, "-m", "Synthetic test fixture").stdout.strip()
        self.git("update-ref", "refs/heads/main", commit)
        self.git("config", "core.hooksPath", ".githooks")

    def git(self, *args, check=True):
        return subprocess.run(["git", *args], cwd=self.root, env=self.environment,
                              check=check, capture_output=True, text=True, timeout=30)

    def write(self, relative, content):
        path = self.root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content, encoding="utf-8")

    def stage_docs(self):
        self.write("docs/spec.md", "# Fixture contract\n\nUpdated arithmetic acceptance criterion.\n")
        self.git("add", "docs/spec.md")

    def hook(self):
        return subprocess.run([sys.executable, ".githooks/pre_commit.py"], cwd=self.root,
                              env=self.environment, capture_output=True, text=True, timeout=60)

    def test_adapter_change_runs_full_suite_and_propagates_failure(self):
        # Check runner arguments and exit propagation without simulating a service.
        self.write("scripts/verify.py", "import sys\nassert '--pure' not in sys.argv\nprint('full-suite-selected')\nsys.exit(7)\n")
        self.git("add", "scripts/verify.py")
        self.write("src/io/adapter.rs", "// adapter contract\n")
        self.git("add", "src/io/adapter.rs")
        result = self.hook()
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("full-suite-selected", result.stdout)
        self.assertIn("VERIFICATION_FAILED", result.stderr)

    def test_failing_staged_code_cannot_be_repaired_by_unstaged_code(self):
        self.write("src/pure/value.py", "assert 2 + 2 == 5\n")
        self.git("add", "src/pure/value.py")
        self.write("src/pure/value.py", "assert 2 + 2 == 4\n")
        self.stage_docs()
        result = self.hook()
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn("VERIFICATION_FAILED", result.stderr)

    def test_valid_code_does_not_require_staged_documentation(self):
        self.write("src/pure/value.py", "assert 3 + 3 == 6\n")
        self.git("add", "src/pure/value.py")
        self.write("docs/spec.md", "# Unstaged update\n")
        result = self.hook()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_documentation_only_change_skips_tests(self):
        self.stage_docs()
        result = self.hook()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Documentation-only change", result.stdout)

    def test_documentation_deletion_does_not_block_valid_code(self):
        self.write("src/pure/value.py", "assert 3 + 3 == 6\n")
        self.git("add", "src/pure/value.py")
        self.git("rm", "docs/spec.md")
        result = self.hook()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_real_git_commit_runs_the_launcher_and_blocks_failure(self):
        previous = self.git("rev-parse", "HEAD").stdout
        self.write("src/pure/value.py", "assert False\n")
        self.git("add", "src/pure/value.py")
        self.stage_docs()
        result = self.git("commit", "-m", "Must be rejected", check=False)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("VERIFICATION_FAILED", result.stderr)
        self.assertEqual(previous, self.git("rev-parse", "HEAD").stdout)

    def test_valid_pure_change_and_docs_commit_successfully(self):
        self.write("src/pure/value.py", "assert 3 + 3 == 6\n")
        self.git("add", "src/pure/value.py")
        self.stage_docs()
        result = self.git("commit", "-m", "Verified fixture", check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(self.git("status", "--porcelain").stdout, "")


if __name__ == "__main__":
    unittest.main()
