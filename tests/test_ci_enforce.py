"""Exercise CI event selection with real Git repositories and detached checkouts."""
from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile
import unittest
import importlib.util
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
SPEC = importlib.util.spec_from_file_location('ci_enforce_tests_subject', ROOT / 'scripts/ci_enforce.py')
ENFORCE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ENFORCE)


class EnforcementModeTests(unittest.TestCase):
    def test_shared_event_policy_propagates_explicit_qualification_only_when_required(self):
        for draft, action, label, expected in ((True, 'synchronize', None, False),
                (False, 'synchronize', None, True), (True, 'labeled', 'qualification', True)):
            event = {'action': action, 'pull_request': {'draft': draft, 'base': {'sha': 'a' * 40}}}
            if label:
                event['label'] = {'name': label}
            with self.subTest(draft=draft, action=action), \
                    patch.dict(os.environ, GITHUB_EVENT_NAME='pull_request'), \
                    patch.object(ENFORCE, 'read_event', return_value=event), \
                    patch('ci_scope.subprocess.check_output', return_value=b'src/io/authority.rs\0'), \
                    patch.object(ENFORCE.subprocess, 'run') as run:
                ENFORCE.main()
            command = run.call_args.args[0]
            self.assertEqual(command[:4], [sys.executable, '.githooks/pre_commit.py', '--base', 'a' * 40])
            self.assertEqual('--qualification' in command, expected)

    def test_bad_metadata_or_unavailable_diff_fails_before_gate_execution(self):
        event = {'action': 'synchronize', 'pull_request': {'draft': 'true', 'base': {'sha': 'a' * 40}}}
        with patch.dict(os.environ, GITHUB_EVENT_NAME='pull_request'), \
                patch.object(ENFORCE, 'read_event', return_value=event), \
                patch.object(ENFORCE.subprocess, 'run') as run:
            with self.assertRaises(ValueError):
                ENFORCE.main()
        run.assert_not_called()


class CiEnforceTests(unittest.TestCase):
    def test_root_push_from_detached_checkout_preserves_staged_tree(self):
        with tempfile.TemporaryDirectory(prefix="kehila-ci-test-") as temporary:
            root = Path(temporary)
            environment = {k: v for k, v in os.environ.items() if not k.startswith("GIT_")}
            environment.update(GIT_AUTHOR_NAME="CI Test", GIT_COMMITTER_NAME="CI Test",
                               GIT_AUTHOR_EMAIL="test@example.invalid",
                               GIT_COMMITTER_EMAIL="test@example.invalid")

            def git(*args):
                return subprocess.check_output(["git", *args], cwd=root, env=environment)

            git("init", "-b", "main")
            (root / "scripts").mkdir()
            (root / "scripts/ci_enforce.py").write_bytes((ROOT / "scripts/ci_enforce.py").read_bytes())
            (root / "scripts/ci_scope.py").write_bytes((ROOT / "scripts/ci_scope.py").read_bytes())
            (root / ".githooks").mkdir()
            # This fixture checks Git plumbing only; it does not simulate physical I/O.
            (root / ".githooks/pre_commit.py").write_text(
                "def verification_scope(paths): return 'full'\n"
                "if __name__ == '__main__':\n"
                " import subprocess, sys\n from pathlib import Path\n"
                " assert '--qualification' in sys.argv, sys.argv\n"
                " assert (Path.cwd() / '.git/HEAD').is_file(), 'Missing HEAD'\n"
                " subprocess.check_call(['git', 'symbolic-ref', 'HEAD'])\n"
                " staged = subprocess.check_output(['git', 'diff', '--cached', '--name-only'])\n"
                " assert b'payload.txt' in staged, staged\n", encoding="utf-8")
            (root / "payload.txt").write_text("root commit payload\n", encoding="utf-8")
            git("add", ".")
            tree = git("write-tree").decode().strip()
            commit = git("commit-tree", tree, "-m", "CI fixture").decode().strip()
            git("update-ref", "refs/heads/main", commit)
            git("checkout", "--detach", commit)
            git("update-ref", "-d", "refs/heads/main")
            event = root / "event.json"
            event.write_text(json.dumps({"before": "0" * 40, 'ref': 'refs/heads/main'}), encoding="utf-8")
            environment["GITHUB_EVENT_PATH"] = str(event)
            environment['GITHUB_EVENT_NAME'] = 'push'
            result = subprocess.run([sys.executable, "scripts/ci_enforce.py"], cwd=root,
                                    env=environment, capture_output=True, text=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(git("rev-parse", "HEAD").decode().strip(), commit)
            self.assertEqual(git("write-tree").decode().strip(), tree)


if __name__ == "__main__":
    unittest.main()
