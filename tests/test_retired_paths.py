"""The retirement backstop distinguishes active inputs from historical records."""
from pathlib import Path
import json
import sys
import tempfile
import unittest
from unittest.mock import patch

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

    def write(self, name, text):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text, encoding='utf-8')
        return path

    def test_retired_free_inputs_and_historical_or_frozen_text_pass(self):
        self.assertEqual(guard.violations(self.root), [])
        historical = 'Historical FerretDB mongodb task_worker go/io/task_api\n'
        self.write('docs/architecture/historical.md', historical)
        self.write('experiments/SP-001/record.json', json.dumps({'historical': historical}))
        self.write('experiments/SP-003/fixtures/frozen-v1.json', json.dumps({'historical': historical}))
        self.assertEqual(guard.violations(self.root), [])

    def test_mongodb_dependency_reintroduction_fails_without_changing_inputs(self):
        self.assertEqual(guard.violations(self.root), [])
        path = self.root / 'Cargo.toml'
        self.write('Cargo.toml', path.read_text(encoding='utf-8')
                   + '\n[workspace.dependencies]\nmongodb = "=2.8.2"\n')
        retained = path.read_bytes()
        self.assertTrue(any('Cargo.toml' in item for item in guard.violations(self.root)))
        self.assertEqual(path.read_bytes(), retained)

    def test_retired_entry_point_and_live_verifier_reference_fail(self):
        self.assertEqual(guard.violations(self.root), [])
        path = self.write('go/io/task_api/go.mod', 'module obsolete\ngo 1.22.0\n')
        self.assertTrue(any('go/io/task_api' in item for item in guard.violations(self.root)))
        path.unlink()
        path.parent.rmdir()
        self.assertEqual(guard.violations(self.root), [])
        self.write('scripts/verify.py', 'command = "tests/integration/task_path.py"\n')
        self.assertTrue(any('scripts/verify.py' in item for item in guard.violations(self.root)))

    def test_missing_required_input_cannot_silently_reduce_scope(self):
        self.assertEqual(guard.violations(self.root), [])
        (self.root / 'Cargo.lock').unlink()
        self.assertTrue(any('Cargo.lock' in item for item in guard.violations(self.root)))

    def test_retired_settings_and_binary_are_rejected_in_active_config(self):
        self.assertEqual(guard.violations(self.root), [])
        for spelling in ('task-api', 'KEHILA_WORKER_URL', 'KEHILA_WORKER_LISTEN_ADDR',
                         'KEHILA_TASK_DB', 'KEHILA_TASK_COLLECTION', 'KEHILA_API_URL'):
            with self.subTest(binding=spelling):
                self.write('config/launcher.json', json.dumps({'binding': spelling}))
                self.assertTrue(any('config/launcher.json' in item
                                    for item in guard.violations(self.root)))

    def test_existing_catalogs_are_checked_without_loading_service_runtime(self):
        self.assertEqual(guard.violations(self.root), [])
        for name, value in (('GO_MODULES', ('go/io/task_api',)),
                            ('SERVICES', {'postgres', 'ferretdb'}),
                            ('PORTS', {'FerretDB': 27017}),
                            ('GROUPS', (('postgres', 'ferretdb'),))):
            with self.subTest(catalog=name), patch.object(guard, name, value):
                self.assertTrue(any(name in item for item in guard.violations(self.root)))
