"""Exercise CI event selection with real Git repositories and detached checkouts."""
from pathlib import Path
import json
import os
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]


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
            (root / ".githooks").mkdir()
            # This fixture checks Git plumbing only; it does not simulate physical I/O.
            (root / ".githooks/pre_commit.py").write_text(
                "import subprocess\nfrom pathlib import Path\n"
                "assert (Path.cwd() / '.git/HEAD').is_file(), 'Missing HEAD'\n"
                "subprocess.check_call(['git', 'symbolic-ref', 'HEAD'])\n"
                "staged = subprocess.check_output(['git', 'diff', '--cached', '--name-only'])\n"
                "assert b'payload.txt' in staged, staged\n", encoding="utf-8")
            (root / "payload.txt").write_text("root commit payload\n", encoding="utf-8")
            git("add", ".")
            tree = git("write-tree").decode().strip()
            commit = git("commit-tree", tree, "-m", "CI fixture").decode().strip()
            git("update-ref", "refs/heads/main", commit)
            git("checkout", "--detach", commit)
            git("update-ref", "-d", "refs/heads/main")
            event = root / "event.json"
            event.write_text(json.dumps({"before": "0" * 40}), encoding="utf-8")
            environment["GITHUB_EVENT_PATH"] = str(event)
            result = subprocess.run([sys.executable, "scripts/ci_enforce.py"], cwd=root,
                                    env=environment, capture_output=True, text=True, timeout=60)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertEqual(git("rev-parse", "HEAD").decode().strip(), commit)
            self.assertEqual(git("write-tree").decode().strip(), tree)


if __name__ == "__main__":
    unittest.main()
