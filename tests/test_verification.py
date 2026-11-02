"""Check which test suite is selected for changed files."""
from pathlib import Path
import importlib.util
import unittest
import subprocess
import tempfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("verification", ROOT / ".githooks/pre_commit.py")
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


class VerificationTests(unittest.TestCase):
    def test_verification_failure_survives_snapshot_cleanup_failure(self):
        primary = subprocess.CalledProcessError(3, ['verifier'])
        cleanup = PermissionError('snapshot locked')
        allocate = POLICY.owned_directory
        with tempfile.TemporaryDirectory() as parent:
            def directory(prefix):
                return allocate(prefix, parent=parent, retry_seconds=0)
            with patch.object(POLICY, 'head_commit', return_value=b'head'), \
                    patch.object(POLICY, 'git', side_effect=[b'tree', b'scripts/check.py\0', b'', b'', b'']), \
                    patch.object(POLICY, 'owned_directory', side_effect=directory), \
                    patch.object(POLICY.subprocess, 'run', side_effect=primary), \
                    patch('temp_work._remove_owned', side_effect=cleanup), \
                    patch('sys.argv', ['pre_commit.py']):
                with self.assertRaises(subprocess.CalledProcessError) as caught:
                    POLICY.main()
        self.assertIs(caught.exception, primary)
        self.assertEqual(primary._cleanup_failures, (('temporary directory', cleanup),))

    def test_documentation_shortcut_still_rejects_a_changed_index(self):
        with patch.object(POLICY, 'head_commit', return_value=b'head'), \
                patch.object(POLICY, 'git', side_effect=[b'tree', b'docs/spec.md\0', b'', b'other-tree']), \
                patch('sys.argv', ['pre_commit.py']):
            self.assertEqual(POLICY.main(), 3)

    def test_documentation_shortcut_still_rejects_a_changed_head(self):
        with patch.object(POLICY, 'head_commit', side_effect=[b'head', b'other-head']), \
                patch.object(POLICY, 'git', side_effect=[b'tree', b'docs/spec.md\0', b'', b'tree']), \
                patch('sys.argv', ['pre_commit.py']):
            self.assertEqual(POLICY.main(), 3)

    def test_only_known_prose_paths_select_documentation_scope(self):
        self.assertEqual(POLICY.verification_scope(['README.md', 'docs/TESTING.md']), 'docs')
        self.assertEqual(POLICY.verification_scope(['.github/pull_request_template.md']), 'docs')
        for paths in ([], ['new-config.toml'], ['docs/fixture.json'],
                      ['README.md', 'scripts/verify.py'], ['.github/workflows/ci.yml'],
                      ['Cargo.lock'], ['src/io/task_worker/src/lib.rs']):
            with self.subTest(paths=paths):
                self.assertEqual(POLICY.verification_scope(paths), 'full')

    def test_engineering_inputs_and_pure_code_keep_tests(self):
        for path in ('docs/engineering/requirements.json',
                     'docs/engineering/STANDARD-PREAMBLE.md',
                     'docs/engineering/ENGINEERING-STANDARD.md',
                     'src/pure/task_contract/src/lib.rs'):
            self.assertEqual(POLICY.verification_scope([path]), 'pure', path)
            self.assertEqual(POLICY.verification_scope(['README.md', path]), 'pure', path)

    def test_live_scope_includes_dependencies_and_verifiers(self):
        for path in ("src/io/x/src/lib.rs", "Cargo.lock", "go/pure/x/go.mod",
                     "packages/x/package.json", "scripts/verify.py", ".githooks/pre_commit.py"):
            self.assertTrue(POLICY.requires_live([path]), path)
        self.assertFalse(POLICY.requires_live(["src/pure/kehila_query/src/lib.rs", "README.md"]))

    def test_source_and_configuration_require_tests(self):
        self.assertTrue(POLICY.requires_tests("src/pure/kehila_query/src/lib.rs"))
        self.assertTrue(POLICY.requires_tests("docker-compose.yml"))
        self.assertTrue(POLICY.requires_tests("pnpm-lock.yaml"))
        self.assertTrue(POLICY.requires_tests("package-lock.json"))
        self.assertFalse(POLICY.requires_tests("docs/TESTING.md"))

    def test_engineering_metadata_cannot_take_documentation_only_bypass(self):
        for path in ("docs/engineering/requirements.json", "docs/engineering/ENGINEERING-STANDARD.md",
                     "docs/engineering/STANDARD-PREAMBLE.md"):
            self.assertTrue(POLICY.requires_tests(path), path)
            self.assertFalse(POLICY.requires_live([path]), path)


if __name__ == "__main__":
    unittest.main()
