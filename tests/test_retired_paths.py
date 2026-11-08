"""The retirement backstop distinguishes active inputs from historical records."""
from pathlib import Path
import json
import io
import sys
import tempfile
import unittest
from unittest.mock import patch
from retired_bindings import RETIRED

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import check_retired_paths as guard


class RetirementGuardTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        for name, text in {
            'Cargo.toml': '[workspace]\nmembers = ["src/pure/*", "src/io/*"]\n',
            'Cargo.lock': 'version = 4\n',
            'go.work': 'go 1.22.0\nuse ./go/pure/sync_contract\n',
            'docker-compose.yml': 'services:\n  postgres:\n    image: postgres:16\n',
            'experiments/SP-003/build.py': 'producer = "project_create_codec"\n',
        }.items():
            self.write(name, text)
        for name in (
                'SP-002/check_seed_oracle.py', 'SP-002/manage.py', 'SP-002/seed_oracle.rs',
                'SP-003/check.py', 'SP-003/run.py', 'SP-003/containers.py',
                'SP-003/main.rs', 'SP-003/codec.rs', 'SP-003/tests.rs'):
            self.write('experiments/' + name, '')

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        return path

    def test_retired_free_inputs_and_historical_or_frozen_text_pass(self):
        self.assertEqual(guard.violations(self.root), [])
        historical = RETIRED['historical']
        self.write('docs/architecture/historical.md', historical)
        self.write('experiments/SP-001/record.json', json.dumps({'historical': historical}))
        self.write('experiments/SP-003/fixtures/frozen-v1.json', json.dumps({'historical': historical}))
        self.assertEqual(guard.violations(self.root), [])

    def test_mongodb_dependency_reintroduction_fails_without_changing_inputs(self):
        self.assertEqual(guard.violations(self.root), [])
        path = self.root / 'Cargo.toml'
        self.write('Cargo.toml', path.read_text(encoding='utf-8')
                   + RETIRED['cargo_dependency'])
        retained = path.read_bytes()
        self.assertTrue(any('Cargo.toml' in item for item in guard.violations(self.root)))
        self.assertEqual(path.read_bytes(), retained)

    def test_retired_entry_point_and_live_verifier_reference_fail(self):
        self.assertEqual(guard.violations(self.root), [])
        path = self.write(RETIRED['go_manifest'], 'module obsolete\ngo 1.22.0\n')
        self.assertTrue(any(RETIRED['go_module'] in item for item in guard.violations(self.root)))
        path.unlink()
        path.parent.rmdir()
        self.assertEqual(guard.violations(self.root), [])
        self.write('scripts/verify.py', RETIRED['verifier_source'])
        self.assertTrue(any('scripts/verify.py' in item for item in guard.violations(self.root)))

    def test_missing_required_input_cannot_silently_reduce_scope(self):
        self.assertEqual(guard.violations(self.root), [])
        (self.root / 'Cargo.lock').unlink()
        self.assertTrue(any('Cargo.lock' in item for item in guard.violations(self.root)))

    def test_retired_settings_and_binary_are_rejected_in_active_config(self):
        self.assertEqual(guard.violations(self.root), [])
        for spelling in RETIRED['settings']:
            with self.subTest(binding=spelling):
                self.write('config/launcher.json', json.dumps({'binding': spelling}))
                self.assertTrue(any('config/launcher.json' in item
                                    for item in guard.violations(self.root)))

    def test_existing_catalogs_are_checked_without_loading_service_runtime(self):
        self.assertEqual(guard.violations(self.root), [])
        for name, value in (('GO_MODULES', (RETIRED['go_module'],)),
                            ('SERVICES', set(RETIRED['service_group'])),
                            ('PORTS', {RETIRED['service']: 27017}),
                            ('GROUPS', (tuple(RETIRED['service_group']),))):
            with self.subTest(catalog=name), patch.object(guard, name, value):
                self.assertTrue(any(name in item for item in guard.violations(self.root)))

    def test_active_unit_and_oracle_inputs_reject_reintroduced_bindings(self):
        for name in ('tests/test_new_verifier.py', 'tests/new_helper.py',
                     'experiments/SP-002/check_seed_oracle.py', 'experiments/SP-002/manage.py',
                     'experiments/SP-002/seed_oracle.rs', 'experiments/SP-003/check.py',
                     'experiments/SP-003/build.py', 'experiments/SP-003/run.py',
                     'experiments/SP-003/containers.py', 'experiments/SP-003/main.rs',
                     'experiments/SP-003/codec.rs', 'experiments/SP-003/tests.rs'):
            with self.subTest(input=name):
                self.assertEqual(guard.violations(self.root), [])
                path = self.write(name, 'binding = ' + repr(RETIRED['settings'][0]))
                self.assertTrue(any(name in item for item in guard.violations(self.root)))
                path.write_text('', encoding='utf-8')

    def test_missing_retained_oracle_input_fails_closed(self):
        self.assertEqual(guard.violations(self.root), [])
        (self.root / 'experiments/SP-003/codec.rs').unlink()
        self.assertTrue(any('experiments/SP-003/codec.rs' in item
                            for item in guard.violations(self.root)))

    def test_negative_data_exception_does_not_exempt_executable_test_code(self):
        self.write('tests/fixtures/retired-bindings.json', json.dumps(RETIRED))
        self.assertEqual(guard.violations(self.root), [])
        self.write('tests/test_healthcheck.py', 'binding = ' + repr(RETIRED['service']))
        self.assertTrue(any('tests/test_healthcheck.py' in item
                            for item in guard.violations(self.root)))

    def test_other_fixture_data_is_not_implicitly_exempt(self):
        self.assertEqual(guard.violations(self.root), [])
        self.write('tests/fixtures/other.json', json.dumps(RETIRED))
        self.assertTrue(any('tests/fixtures/other.json' in item
                            for item in guard.violations(self.root)))

    def test_invalid_utf8_reports_its_path_and_preserves_other_violations(self):
        self.assertEqual(guard.violations(self.root), [])
        path = self.write('scripts/a_invalid.py', '')
        path.write_bytes(b'\xff')
        self.write('scripts/z_legacy.py', RETIRED['verifier_source'])
        errors = guard.violations(self.root)
        self.assertIn('scripts/a_invalid.py: active input is not UTF-8', errors)
        self.assertTrue(any('scripts/z_legacy.py' in item for item in errors))
        self.assertEqual(path.read_bytes(), b'\xff')

    def test_invalid_utf8_cli_returns_failure_with_actionable_diagnostic(self):
        path = self.write('config/invalid.json', '')
        path.write_bytes(b'\xff')
        with patch.object(guard, 'ROOT', self.root), patch.object(sys, 'stderr', io.StringIO()) as error:
            self.assertEqual(guard.main(), 1)
        self.assertIn('config/invalid.json: active input is not UTF-8', error.getvalue())
