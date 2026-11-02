"""Verify staged test execution and hook dispatch in temporary Git repositories."""
from pathlib import Path
import os
import shutil
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
        for relative in (".githooks/pre_commit.py", ".githooks/pre-commit",
                         "scripts/temp_work.py", "scripts/cleanup_errors.py",
                         "scripts/verification_settings.py", "config/verification-defaults.json"):
            self.write(relative, (ROOT / relative).read_text(encoding="utf-8"))
        hook = self.root / ".githooks/pre-commit"
        hook.chmod(hook.stat().st_mode | stat.S_IXUSR)
        self.write("src/pure/value.py", "assert 2 + 2 == 4\n")
        self.write("docs/spec.md", "# Fixture contract\n\nArithmetic is pure.\n")
        self.write("scripts/verify.py", "import subprocess, sys\nassert '--pure' in sys.argv\nsubprocess.run([sys.executable, 'src/pure/value.py'], check=True)\n")
        # This fixture exercises Git plumbing, not the engine execution contract.
        self.write("scripts/container_verify.py", "import runpy\nrunpy.run_path('scripts/verify.py', run_name='__main__')\n")
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

    def test_checkout_local_override_is_rejected_before_snapshot_verification(self):
        self.write('local-settings.json', '{"machine_start_seconds": 300}')
        self.environment['KEHILA_VERIFICATION_CONFIG'] = str(self.root / 'local-settings.json')
        self.write('src/pure/value.py', 'assert 3 + 3 == 6\n')
        self.git('add', 'src/pure/value.py')
        result = self.hook()
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('outside the checkout', result.stderr)
        self.assertNotIn('Staged snapshot verified', result.stdout)

    def test_unsupported_modes_cannot_take_the_documentation_shortcut(self):
        self.write('docs/unsupported.md', 'link-target')
        blob = self.git('hash-object', '-w', 'docs/unsupported.md').stdout.strip()
        commit = self.git('rev-parse', 'HEAD').stdout.strip()
        for mode, identity in (('120000', blob), ('160000', commit)):
            with self.subTest(mode=mode):
                self.git('update-index', '--add', '--cacheinfo',
                         mode + ',' + identity + ',docs/unsupported.md')
                result = self.hook()
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('unsupported', result.stderr)
                self.git('update-index', '--force-remove', 'docs/unsupported.md')

    def test_unborn_index_uses_an_immutable_empty_baseline(self):
        self.write('scripts/verify.py', "print('unborn-snapshot-verified')\n")
        self.git('add', 'scripts/verify.py')
        self.git('update-ref', '-d', 'refs/heads/main')
        result = self.hook()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('unborn-snapshot-verified', result.stdout)

    def test_captured_tree_controls_scope_and_export_during_index_ABA(self):
        for boundary in ('scope', 'export'):
            for valid in (True, False):
                with self.subTest(boundary=boundary, captured_source_valid=valid):
                    self.write('src/pure/value.py', 'assert ' + str(valid) + '\n')
                    self.git('add', 'src/pure/value.py')
                    captured = self.git('write-tree').stdout.strip()
                    self.write('src/pure/value.py', 'assert ' + str(not valid) + '\n')
                    self.write('src/io/adapter.rs', '// alternate tree requires full scope\n')
                    self.git('add', 'src/pure/value.py', 'src/io/adapter.rs')
                    alternate = self.git('write-tree').stdout.strip()
                    self.git('read-tree', captured)
                    script = self.root / 'index-race.py'
                    script.write_text(f"""import importlib.util, sys
from pathlib import Path
spec = importlib.util.spec_from_file_location('gate', Path('.githooks/pre_commit.py'))
gate = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gate)
real_git = gate.git
boundary = {boundary!r}
def racing_git(*args, **kwargs):
    operation = 'diff' if boundary == 'scope' else 'checkout-index'
    if args[0] == operation:
        real_git('read-tree', {alternate!r})
        try:
            return real_git(*args, **kwargs)
        finally:
            real_git('read-tree', {captured!r})
    return real_git(*args, **kwargs)
gate.git = racing_git
sys.argv = ['pre_commit.py']
sys.exit(gate.main())
""", encoding='utf-8')
                    result = subprocess.run([sys.executable, str(script)], cwd=self.root,
                                            env=self.environment, capture_output=True,
                                            text=True, timeout=60)
                    if valid:
                        self.assertEqual(result.returncode, 0, result.stderr)
                    else:
                        self.assertNotEqual(result.returncode, 0)
                    self.assertEqual(self.git('write-tree').stdout.strip(), captured)

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

    def test_build_cache_respects_external_directory_and_preserves_default(self):
        with tempfile.TemporaryDirectory(prefix="kehila-external-cache-") as external:
            self.write("scripts/verify.py", """from pathlib import Path
import os
cache = Path(os.environ['CARGO_TARGET_DIR'])
cache.mkdir(parents=True, exist_ok=True)
(cache / 'verified-cache').write_text('built from staged source')
""")
            self.git("add", "scripts/verify.py")
            for configured in (Path(external), None):
                with self.subTest(external=configured is not None):
                    if configured is None:
                        self.environment.pop("CARGO_TARGET_DIR", None)
                    else:
                        self.environment["CARGO_TARGET_DIR"] = str(configured)
                    result = self.hook()
                    self.assertEqual(result.returncode, 0, result.stderr)
                    cache = configured if configured is not None else self.root / "target"
                    self.assertEqual((cache / "verified-cache").read_text(), "built from staged source")
                    if configured is not None:
                        self.assertFalse((self.root / "target").exists())

    def test_verifier_receives_the_deadline_and_can_clean_up_after_work_timeout(self):
        self.write('scripts/verify.py', '''import subprocess, sys
assert sys.argv[sys.argv.index('--timeout') + 1] == '600'
try:
    subprocess.run([sys.executable, '-c', 'import time; time.sleep(5)'], check=True, timeout=0.2)
finally:
    print('owner-cleanup-after-work-timeout', flush=True)
''')
        self.git('add', 'scripts/verify.py')
        result = self.hook()
        self.assertEqual(result.returncode, 3, result.stderr)
        self.assertIn('owner-cleanup-after-work-timeout', result.stdout)

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

    @unittest.skipUnless(os.name == "posix", "POSIX interpreter-only PATH fixture")
    def test_real_commit_with_python3_only(self):
        self.commit_with_interpreter_only("python3")

    @unittest.skipUnless(os.name == "posix", "POSIX interpreter-only PATH fixture")
    def test_real_commit_with_python_only(self):
        self.commit_with_interpreter_only("python")

    def commit_with_interpreter_only(self, interpreter):
        self.stage_docs()
        bin_dir = self.root / "bin"
        bin_dir.mkdir()
        for name, executable in [("git", shutil.which("git")), ("sh", shutil.which("sh")),
                                 (interpreter, sys.executable)]:
            (bin_dir / name).symlink_to(executable)
        self.environment["PATH"] = str(bin_dir)
        result = self.git("commit", "-m", "Verify interpreter dispatch", check=False)
        self.assertEqual(result.returncode, 0, result.stderr)

    @unittest.skipUnless(os.name == "nt", "Windows batch launcher")
    def test_batch_forwards_arguments_and_preserves_exit_code(self):
        self.write(".githooks/pre-commit.bat", (ROOT / ".githooks/pre-commit.bat").read_text())
        self.write(".githooks/pre_commit.py", "import sys\nprint(repr(sys.argv[1:]))\nsys.exit(7)\n")
        result = subprocess.run(
            ["cmd.exe", "/d", "/c", ".githooks\\pre-commit.bat", "--base", "argument with spaces"],
            cwd=self.root, env=self.environment, capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode, 7, result.stderr)
        self.assertIn("['--base', 'argument with spaces']", result.stdout)

    @unittest.skipUnless(os.name == "nt", "Windows batch launcher")
    def test_batch_missing_interpreter_has_actionable_error(self):
        self.write(".githooks/pre-commit.bat", (ROOT / ".githooks/pre-commit.bat").read_text())
        environment = dict(self.environment, PATH="")
        result = subprocess.run(
            [str(Path(os.environ["SystemRoot"]) / "System32/cmd.exe"), "/d", "/c", ".githooks\\pre-commit.bat"],
            cwd=self.root, env=environment, capture_output=True, text=True, timeout=15,
        )
        self.assertEqual(result.returncode, 127)
        self.assertIn("Python 3.10+ is required", result.stderr)


if __name__ == "__main__":
    unittest.main()
