"""Regression coverage for enforceable metadata, not compliance conclusions."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('engineering', ROOT / 'scripts/check_engineering.py')
ENGINEERING = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ENGINEERING)


class RegisterTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads((ROOT / ENGINEERING.REGISTER).read_text(encoding='utf-8'))

    def test_real_register_and_generated_document(self):
        self.assertEqual(ENGINEERING.validate(self.data), [])
        self.assertEqual(ENGINEERING.render(self.data), (ROOT / ENGINEERING.DOCUMENT).read_text(encoding='utf-8'))

    def test_duplicate_and_invalid_ids_fail(self):
        self.data['requirements'].append(copy.deepcopy(self.data['requirements'][0]))
        self.assertTrue(any('duplicate ID' in e for e in ENGINEERING.validate(self.data)))
        self.data['requirements'][0]['id'] = 'GOV-1'
        self.assertTrue(any('invalid or duplicate' in e for e in ENGINEERING.validate(self.data)))

    def test_broken_backlog_anchor_fails(self):
        self.data['requirements'][0]['work'] = ['docs/product/backlog.md#b-999']
        self.assertTrue(any('missing explicit anchor' in e for e in ENGINEERING.validate(self.data)))

    def test_ambiguous_reference_anchor_fails(self):
        with patch.object(Path, 'read_text', return_value='<a id="b-019"></a>\n<a id="b-019"></a>'):
            self.assertIn('duplicate explicit anchor', ENGINEERING.reference_error(ROOT, 'docs/product/backlog.md#b-019'))

    def test_unknown_source_or_missing_backlog_fails(self):
        self.data['requirements'][0]['sources'] = ['Invented']
        self.data['requirements'][0]['work'] = ['README.md']
        errors = ENGINEERING.validate(self.data)
        self.assertTrue(any('unknown source' in e for e in errors))
        self.assertTrue(any('backlog destination' in e for e in errors))

    def test_implemented_status_requires_evidence(self):
        self.data['requirements'][0]['status'] = 'satisfied'
        self.data['requirements'][0]['evidence'] = []
        self.assertTrue(any('requires evidence' in e for e in ENGINEERING.validate(self.data)))

    def test_missing_file_and_escape_fail(self):
        for reference in ('docs/missing.md', '../outside.md', str(ROOT.parent / 'outside.md')):
            with self.subTest(reference=reference):
                self.assertIsNotNone(ENGINEERING.reference_error(ROOT, reference))

    def test_gate_normative_status_and_schema_fail_closed(self):
        for field, value in [('gate', True), ('gate', 6), ('status', 'waived'),
                             ('statement', 'We aspire to quality'), ('enforcement', ''),
                             ('sources', []), ('work', None)]:
            data = copy.deepcopy(self.data)
            data['requirements'][0][field] = value
            with self.subTest(field=field, value=value):
                self.assertTrue(ENGINEERING.validate(data))
        self.assertTrue(ENGINEERING.validate({'version': 1}))

    def test_stale_document_fails_without_mutating_it(self):
        original = Path.read_text
        def read(path, **kwargs):
            return 'stale' if path == ROOT / ENGINEERING.DOCUMENT else original(path, **kwargs)
        with patch.object(sys, 'argv', ['check_engineering.py']), patch.object(Path, 'read_text', read):
            with self.assertRaisesRegex(ValueError, 'stale'):
                ENGINEERING.main()

    def test_cumulative_gate_refuses_partial_evidence(self):
        for gate in (1, 2, 3, 4, 5):
            with self.subTest(gate=gate), patch.object(sys, 'argv', ['check_engineering.py', '--gate', str(gate)]):
                with self.assertRaisesRegex(ValueError, f'M{gate} missing evidence'):
                    ENGINEERING.main()

    def test_shared_matrix_includes_enforcement_in_both_modes(self):
        spec = importlib.util.spec_from_file_location('verify_engineering', ROOT / 'scripts/verify.py')
        verify = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(verify)
        for args in ([], ['--pure']):
            with patch.object(sys, 'argv', ['verify.py', *args]), patch.object(verify.subprocess, 'run') as run:
                verify.main()
                commands = [call.args[0] for call in run.call_args_list]
                self.assertIn([sys.executable, 'scripts/check_engineering.py'], commands)
                self.assertIn([sys.executable, 'scripts/go_static.py'], commands)


class GoStaticTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        sys.path.insert(0, str(ROOT / 'scripts'))
        try:
            spec = importlib.util.spec_from_file_location('go_static_checks', ROOT / 'scripts/go_static.py')
            cls.module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(cls.module)
        finally:
            sys.path.pop(0)

    def test_gofmt_violation_blocks_vet(self):
        with patch.object(self.module.subprocess, 'run', return_value=subprocess.CompletedProcess([], 0, 'go/io/task_api/server.go\n')) as run:
            with self.assertRaisesRegex(RuntimeError, 'Run gofmt'):
                self.module.main()
            self.assertEqual(run.call_count, 1)

    def test_vet_failure_is_not_swallowed(self):
        with patch.object(self.module.subprocess, 'run', side_effect=[subprocess.CompletedProcess([], 0, ''), subprocess.CalledProcessError(1, ['go', 'vet'])]) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                self.module.main()
            self.assertEqual(run.call_args.args[0], ['go', 'vet', *self.module.GO_PACKAGES])
