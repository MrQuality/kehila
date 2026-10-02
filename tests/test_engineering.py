"""Regression coverage for enforceable metadata, not compliance conclusions."""
import copy
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
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
        preamble = (ROOT / ENGINEERING.PREAMBLE).read_text(encoding='utf-8')
        self.assertEqual(ENGINEERING.render(self.data, preamble),
                         (ROOT / ENGINEERING.DOCUMENT).read_text(encoding='utf-8'))

    def test_preamble_is_authored_markdown_not_register_data(self):
        self.assertNotIn('introduction', self.data)
        rendered = ENGINEERING.render(self.data, '# Authored preamble\n\nContext.\n')
        self.assertTrue(rendered.startswith('# Authored preamble\n\nContext.\n'))
        self.assertIn('## Requirement register', rendered)

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
        for field, value in [('gate', True), ('gate', 'E6'), ('gate', 'M1'),
                             ('gate', 1), ('status', 'waived'),
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

    def write_fixture(self, root):
        """Small valid register with real local evidence and backlog references."""
        data = copy.deepcopy(self.data)
        data['requirements'] = [data['requirements'][0]]
        row = data['requirements'][0]
        row['evidence'] = ['evidence.md']
        row['work'] = ['docs/product/backlog.md#b-019']
        (root / 'docs/product').mkdir(parents=True)
        (root / 'docs/product/backlog.md').write_text('<a id="b-019"></a>', encoding='utf-8')
        (root / 'evidence.md').write_text('Bounded fixture evidence.', encoding='utf-8')
        (root / ENGINEERING.REGISTER).parent.mkdir(parents=True)
        (root / ENGINEERING.REGISTER).write_text(json.dumps(data), encoding='utf-8')
        return data

    def test_write_regenerates_repeatably_and_preserves_authored_preamble(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = self.write_fixture(root)
            authored = b'# Authored context\n\nMaintain this prose separately.\n'
            (root / ENGINEERING.PREAMBLE).write_bytes(authored)
            document = root / ENGINEERING.DOCUMENT
            document.write_text('stale generated output', encoding='utf-8')
            with patch.object(ENGINEERING, 'ROOT', root), \
                    patch.object(sys, 'argv', ['check_engineering.py', '--write']):
                ENGINEERING.main()
                first = document.read_bytes()
                self.assertEqual(first.decode(), ENGINEERING.render(data, authored.decode()))
                ENGINEERING.main()
                self.assertEqual(document.read_bytes(), first)
                self.assertEqual((root / ENGINEERING.PREAMBLE).read_bytes(), authored)

    def test_write_refuses_invalid_input_without_clobbering_document(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = self.write_fixture(root)
            document = root / ENGINEERING.DOCUMENT
            original = b'Previous generated evidence\n'
            document.write_bytes(original)
            (root / ENGINEERING.PREAMBLE).write_text('# Context\n', encoding='utf-8')
            data['requirements'].append(copy.deepcopy(data['requirements'][0]))
            (root / ENGINEERING.REGISTER).write_text(json.dumps(data), encoding='utf-8')
            with patch.object(ENGINEERING, 'ROOT', root), \
                    patch.object(sys, 'argv', ['check_engineering.py', '--write']):
                with self.assertRaisesRegex(ValueError, 'duplicate ID'):
                    ENGINEERING.main()
                self.assertEqual(document.read_bytes(), original)

    def test_cumulative_gate_refuses_partial_evidence(self):
        for gate in ('E1', 'E2', 'E3', 'E4', 'E5'):
            with self.subTest(gate=gate), patch.object(sys, 'argv', ['check_engineering.py', '--gate', str(gate)]):
                with self.assertRaisesRegex(ValueError, f'{gate} missing evidence'):
                    ENGINEERING.main()

    def test_cli_rejects_product_and_unqualified_numeric_gate_names(self):
        for gate in ('M1', '1'):
            with self.subTest(gate=gate), \
                    patch.object(sys, 'argv', ['check_engineering.py', '--gate', gate]), \
                    patch.object(sys, 'stderr'):
                with self.assertRaises(SystemExit) as result:
                    ENGINEERING.main()
                self.assertEqual(result.exception.code, 2)

    def test_gate_blocks_only_required_unsatisfied_rows(self):
        rows = [
            {'id': 'REQ-001', 'gate': 'E1', 'obligation': 'required', 'status': 'partial'},
            {'id': 'REC-001', 'gate': 'E1', 'obligation': 'recommended', 'status': 'planned'},
            {'id': 'OPT-001', 'gate': 'E1', 'obligation': 'optional', 'status': 'planned'},
            {'id': 'REQ-002', 'gate': 'E2', 'obligation': 'required', 'status': 'planned'},
        ]
        data = {'requirements': rows}
        self.assertEqual(ENGINEERING.gate_blockers(data, 'E1'), ['REQ-001'])
        rows[0]['status'] = 'satisfied'
        self.assertEqual(ENGINEERING.gate_blockers(data, 'E1'), [])
        self.assertEqual(ENGINEERING.gate_blockers(data, 'E2'), ['REQ-002'])

    def test_obligation_is_explicit_and_mixed_strength_rows_fail(self):
        for obligation in ('unknown', None, 'recommended', 'optional'):
            data = copy.deepcopy(self.data)
            data['requirements'][0]['obligation'] = obligation
            with self.subTest(obligation=obligation):
                self.assertTrue(ENGINEERING.validate(data))

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
        result = subprocess.CompletedProcess([], 0, 'go/io/task_api/server.go\n')
        with patch.object(self.module, 'go_files', return_value=['go/io/task_api/server.go']), \
                patch.object(self.module.subprocess, 'run', return_value=result) as run:
            with self.assertRaisesRegex(RuntimeError, 'Run gofmt'):
                self.module.main()
            self.assertEqual(run.call_count, 1)

    def test_vet_failure_is_not_swallowed(self):
        results = [subprocess.CompletedProcess([], 0, ''),
                   subprocess.CalledProcessError(1, ['go', 'vet'])]
        with patch.object(self.module, 'go_files', return_value=['go/io/task_api/server.go']), \
                patch.object(self.module.subprocess, 'run', side_effect=results) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                self.module.main()
            self.assertEqual(run.call_args.args[0], ['go', 'vet', *self.module.GO_PACKAGES])

    def test_omitted_module_blocks_static_checks(self):
        with patch.object(self.module, 'go_files', side_effect=RuntimeError('module omitted')), \
                patch.object(self.module.subprocess, 'run') as run:
            with self.assertRaisesRegex(RuntimeError, 'module omitted'):
                self.module.main()
            run.assert_not_called()
