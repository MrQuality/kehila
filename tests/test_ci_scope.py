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


def pr_event(*, draft=True, action='synchronize', base='a' * 40, label=None):
    event = {'action': action, 'pull_request': {'draft': draft, 'base': {'sha': base}}}
    if label is not None:
        event['label'] = {'name': label}
    return event


def push_event(base='a' * 40):
    return {'before': base, 'ref': 'refs/heads/main'}


class CiScopeTests(unittest.TestCase):
    def choose(self, event, *, name='pull_request', paths=b'src/io/authority.rs\0'):
        with patch.object(SCOPE.subprocess, 'check_output', return_value=paths):
            return SCOPE.choose_scope(event, ROOT, event_name=name)

    def test_draft_updates_select_development_and_final_requests_select_full(self):
        for action in ('opened', 'synchronize', 'reopened', 'converted_to_draft'):
            self.assertEqual(self.choose(pr_event(action=action)), 'development')
        self.assertEqual(self.choose(pr_event(draft=False)), 'full')
        self.assertEqual(self.choose(push_event(), name='push'), 'full')
        self.assertEqual(self.choose(pr_event(action='ready_for_review', draft=False)), 'full')

    def test_explicit_label_qualifies_even_prose_or_empty_diff(self):
        for paths in (b'README.md\0', b''):
            self.assertEqual(self.choose(pr_event(action='labeled', label='qualification'), paths=paths), 'full')
        event = pr_event()
        event['pull_request']['labels'] = [{'name': 'qualification'}]
        self.assertEqual(self.choose(event), 'development')
        self.assertEqual(self.choose(pr_event(action='labeled', label='other')), 'development')

    def test_known_prose_shortcut_still_requires_a_valid_classified_diff(self):
        self.assertEqual(self.choose(pr_event(), paths=b'README.md\0docs/TESTING.md\0'), 'docs')
        for path in (b'docs/engineering/requirements.json\0', b'.github/workflows/ci.yml\0',
                     b'unknown.toml\0', b'src/pure/value.rs\0', b''):
            self.assertEqual(self.choose(pr_event(), paths=path), 'development')
            self.assertEqual(self.choose(pr_event(draft=False), paths=path), 'full')

    def test_invalid_events_fail_instead_of_skipping_or_starting_full_work(self):
        invalid = [(None, 'pull_request'), ([], 'pull_request'), ({}, 'pull_request'),
                   (pr_event(), 'workflow_dispatch'), (pr_event(), 'push'),
                   (pr_event(action='edited'), 'pull_request'),
                   (pr_event(action='ready_for_review'), 'pull_request'),
                   (pr_event(draft=False, action='converted_to_draft'), 'pull_request'),
                   (pr_event(action='labeled'), 'pull_request'),
                   ({'before': 'a' * 40, 'ref': 'refs/heads/other'}, 'push')]
        invalid.extend((pr_event(draft=value), 'pull_request') for value in (None, 'true', 1, [], {}))
        invalid.extend((pr_event(base=value), 'pull_request') for value in (None, '--invalid', '0' * 40, 123))
        for event, name in invalid:
            with self.subTest(event=event, name=name), self.assertRaises(ValueError):
                self.choose(event, name=name)

    def test_diff_failures_are_selection_errors(self):
        for error in (OSError('git unavailable'), subprocess.TimeoutExpired(['git'], 30),
                      subprocess.CalledProcessError(1, ['git']),
                      UnicodeDecodeError('utf-8', b'\xff', 0, 1, 'invalid path')):
            with self.subTest(error=type(error).__name__), \
                    patch.object(SCOPE.subprocess, 'check_output', side_effect=error):
                with self.assertRaises(ValueError):
                    SCOPE.choose_scope(pr_event(), ROOT, event_name='pull_request')

    def test_explicit_qualification_rejects_an_unavailable_base_before_scope_output(self):
        with patch.object(SCOPE.subprocess, 'check_output', side_effect=subprocess.CalledProcessError(1, ['git'])):
            with self.assertRaises(ValueError):
                SCOPE.choose_scope(pr_event(action='labeled', label='qualification'), ROOT,
                                   event_name='pull_request')

    def test_root_main_push_explicitly_selects_qualification(self):
        self.assertEqual(self.choose(push_event('0' * 40), name='push'), 'full')

    def test_malformed_event_does_not_write_a_scope_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event = root / 'event.json'
            event.write_text('{invalid', encoding='utf-8')
            output = root / 'output.txt'
            with patch.dict(os.environ, GITHUB_EVENT_PATH=str(event), GITHUB_OUTPUT=str(output),
                            GITHUB_EVENT_NAME='pull_request'), self.assertRaises(ValueError):
                SCOPE.main()
            self.assertFalse(output.exists())

    def test_documentation_selection_writes_explicit_output(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            event = root / 'event.json'
            event.write_text(json.dumps(pr_event()), encoding='utf-8')
            output = root / 'output.txt'
            with patch.dict(os.environ, GITHUB_EVENT_PATH=str(event), GITHUB_OUTPUT=str(output),
                            GITHUB_EVENT_NAME='pull_request'), \
                    patch.object(SCOPE.subprocess, 'check_output', return_value=b'README.md\0'):
                SCOPE.main()
            self.assertEqual(output.read_text(encoding='utf-8'), 'scope=docs\n')

    def test_real_git_renames_deletions_and_pr_base_precedence(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            environment = {k: v for k, v in os.environ.items() if not k.startswith('GIT_')}
            environment.update(GIT_AUTHOR_NAME='Scope Test', GIT_COMMITTER_NAME='Scope Test',
                               GIT_AUTHOR_EMAIL='test@example.invalid', GIT_COMMITTER_EMAIL='test@example.invalid')
            def git(*args):
                return subprocess.check_output(['git', *args], cwd=root, env=environment)
            git('init', '-b', 'main')
            (root / 'README.md').write_text('Original\n', encoding='utf-8')
            (root / 'src/pure').mkdir(parents=True)
            (root / 'src/pure/example.rs').write_text('fn example() {}\n', encoding='utf-8')
            def commit():
                git('add', '--all')
                sha = git('commit-tree', git('write-tree').decode().strip(), '-m', 'Scope fixture').decode().strip()
                git('update-ref', 'refs/heads/main', sha)
                return sha
            base = commit()
            event = pr_event(base=base)
            event['before'] = '0' * 40
            (root / 'README.md').write_text('Updated\n', encoding='utf-8')
            commit()
            with patch.dict(os.environ, environment, clear=True):
                self.assertEqual(SCOPE.choose_scope(event, root, event_name='pull_request'), 'docs')
                (root / 'docs').mkdir()
                (root / 'src/pure/example.rs').rename(root / 'docs/example.md')
                commit()
                self.assertEqual(SCOPE.choose_scope(event, root, event_name='pull_request'), 'development')
                (root / 'docs/example.md').unlink()
                commit()
                self.assertEqual(SCOPE.choose_scope(event, root, event_name='pull_request'), 'development')


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
            [sys.executable, 'scripts/check_retired_paths.py'],
            [sys.executable, 'scripts/check_engineering.py']])
        self.assertTrue(all(call.kwargs['check'] for call in run.call_args_list))

    def test_documentation_failure_propagates(self):
        with patch.object(sys, 'argv', ['verify.py', '--docs']), \
                patch.object(self.verify.subprocess, 'run',
                             side_effect=subprocess.CalledProcessError(1, ['python'])):
            with self.assertRaises(subprocess.CalledProcessError):
                self.verify.main()
