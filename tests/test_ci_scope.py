"""CI classification uses the hook policy and fails closed on uncertain bases."""
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('ci_scope', ROOT / 'scripts/ci_scope.py')
SCOPE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(SCOPE)


class CiScopeTests(unittest.TestCase):
    def test_malformed_event_writes_explicit_full_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event = root / 'event.json'
            event.write_text('{invalid', encoding='utf-8')
            output = root / 'output.txt'
            with patch.dict(os.environ, GITHUB_EVENT_PATH=str(event), GITHUB_OUTPUT=str(output)):
                SCOPE.main()
            self.assertEqual(output.read_text(encoding='utf-8'), 'scope=full\n')

    def test_documentation_selection_writes_explicit_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event = root / 'event.json'
            event.write_text(json.dumps({'before': 'a' * 40}), encoding='utf-8')
            output = root / 'output.txt'
            with patch.dict(os.environ, GITHUB_EVENT_PATH=str(event), GITHUB_OUTPUT=str(output)), \
                    patch.object(SCOPE.subprocess, 'check_output', return_value=b'README.md\0'):
                SCOPE.main()
            self.assertEqual(output.read_text(encoding='utf-8'), 'scope=docs\n')

    def test_uncertain_root_empty_or_missing_base_selects_full(self):
        for event in ({}, {'before': '0' * 40}, {'before': '--invalid'},
                      {'before': 'a' * 40}):
            with self.subTest(event=event), patch.object(SCOPE.subprocess, 'check_output',
                    side_effect=subprocess.CalledProcessError(1, ['git'])):
                self.assertEqual(SCOPE.choose_scope(event, ROOT), 'full')

    def test_malformed_event_shapes_and_git_failures_select_full(self):
        for event in (None, [], 'invalid', {'pull_request': []},
                      {'pull_request': {'base': 'invalid'}}, {'before': 123}):
            with self.subTest(event=event):
                self.assertEqual(SCOPE.choose_scope(event, ROOT), 'full')
        for error in (OSError('git unavailable'),
                      subprocess.TimeoutExpired(['git'], 30),
                      UnicodeDecodeError('utf-8', b'\xff', 0, 1, 'invalid path')):
            with self.subTest(error=type(error).__name__), \
                    patch.object(SCOPE.subprocess, 'check_output', side_effect=error):
                self.assertEqual(SCOPE.choose_scope({'before': 'a' * 40}, ROOT), 'full')

    def test_real_git_renames_deletions_and_pr_base_precedence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
            environment.update(GIT_AUTHOR_NAME='Scope Test', GIT_COMMITTER_NAME='Scope Test',
                               GIT_AUTHOR_EMAIL='test@example.invalid',
                               GIT_COMMITTER_EMAIL='test@example.invalid')

            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, env=environment)

            git('init', '-b', 'main')
            (root / 'README.md').write_text('Original\n', encoding='utf-8')
            (root / 'src/pure').mkdir(parents=True)
            (root / 'src/pure/example.rs').write_text('fn example() {}\n', encoding='utf-8')

            def commit():
                git('add', '--all')
                tree = git('write-tree').decode().strip()
                sha = git('commit-tree', tree, '-m', 'Scope fixture').decode().strip()
                git('update-ref', 'refs/heads/main', sha)
                return sha

            base = commit()
            event = {'pull_request': {'base': {'sha': base}}, 'before': '0' * 40}
            (root / 'README.md').write_text('Updated\n', encoding='utf-8')
            commit()
            with patch.dict(os.environ, environment, clear=True):
                self.assertEqual(SCOPE.choose_scope(event, root), 'docs')
                self.assertEqual(SCOPE.choose_scope({'before': base}, root), 'docs')
                (root / 'docs').mkdir()
                (root / 'src/pure/example.rs').rename(root / 'docs/example.md')
                commit()
                self.assertEqual(SCOPE.choose_scope(event, root), 'full')
                (root / 'docs/example.md').unlink()
                commit()
                self.assertEqual(SCOPE.choose_scope(event, root), 'full')

    def test_metadata_does_not_take_ci_documentation_shortcut(self):
        for path in (b'docs/engineering/requirements.json\0',
                     b'docs/engineering/STANDARD-PREAMBLE.md\0',
                     b'docs/engineering/ENGINEERING-STANDARD.md\0',
                     b'.githooks/pre_commit.py\0', b'go/io/task_api/server.go\0', b''):
            with self.subTest(path=path), \
                    patch.object(SCOPE.subprocess, 'check_output', return_value=path):
                self.assertEqual(SCOPE.choose_scope({'before': 'a' * 40}, ROOT), 'full')


class DocumentationMatrixTests(unittest.TestCase):
    def setUp(self):
        spec = importlib.util.spec_from_file_location('docs_verify', ROOT / 'scripts/verify.py')
        self.verify = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.verify)

    def test_documentation_matrix_retains_python_and_metadata_but_no_compilers(self):
        with patch.object(sys, 'argv', ['verify.py', '--docs']), \
                patch.object(self.verify.subprocess, 'run') as run:
            self.verify.main()
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands, [
            [sys.executable, '-m', 'unittest', 'discover', '-s', 'tests', '-v'],
            [sys.executable, 'scripts/check_branding.py'],
            [sys.executable, 'scripts/check_engineering.py']])
        self.assertTrue(all(call.kwargs['check'] for call in run.call_args_list))

    def test_documentation_failure_propagates(self):
        with patch.object(sys, 'argv', ['verify.py', '--docs']), \
                patch.object(self.verify.subprocess, 'run',
                             side_effect=subprocess.CalledProcessError(1, ['python'])):
            with self.assertRaises(subprocess.CalledProcessError):
                self.verify.main()
