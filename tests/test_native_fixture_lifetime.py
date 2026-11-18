"""Completed disposable fixture retirement; no engine is contacted by these tests."""
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from test_container_native import Executor
import native_fixture_environment as fixture


class LifetimeExecutor(Executor):
    def __init__(self, source):
        super().__init__()
        self.source = source


class LifetimeTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        setting = patch.object(fixture.tempfile, 'gettempdir', return_value=temporary.name)
        setting.start()
        self.addCleanup(setting.stop)
        self.source = self.root / 'source'
        folder = self.source / 'tests/integration'
        folder.mkdir(parents=True)
        original = Path(__file__).resolve().parents[1] / 'tests/integration'
        for name in ('native_item_mutation_state.sql', 'native_selection_state.sql'):
            (folder / name).write_bytes((original / name).read_bytes())
        self.executor = LifetimeExecutor(self.source)
        self.environment = fixture.NativeEnvironment(self.executor,
            databases=('native_item_mutation', 'native_selection', 'native_auth_owned'),
            observers=('native_item_mutation_state.sql', 'native_selection_state.sql'),
            selection={'scope':'full-native', 'selected_items':['item-mutation','selection'],
                       'observed_items':[]})

    def observe(self, active, name='native_item_mutation'):
        mode = 'item-mutation' if name == 'native_item_mutation' else 'selection'
        observer = 'native_item_mutation_state.sql' if mode == 'item-mutation' else 'native_selection_state.sql'
        return active.observe_database(name, observer, mode + '-observation.txt')

    def drops(self):
        return [args for args in self.executor.events if args[0] == 'exec' and 'dropdb' in args]

    def test_only_successful_declared_observation_can_release_its_original_database(self):
        with self.environment.owned() as active:
            self.assertEqual(self.observe(active), 'DO\n')
            witness = active.folder / 'item-mutation-observation.txt'
            original = witness.read_bytes()
            active.retire_database('native_item_mutation')
            self.assertNotIn('native_item_mutation', self.executor.databases)
            self.assertIn('native_selection', self.executor.databases)
            self.assertIn('native_auth_owned', self.executor.databases)
            self.assertEqual(witness.read_bytes(), original)
            self.assertEqual(len(self.drops()), 1)
            self.assertNotIn('--force', self.drops()[0])
            self.assertNotIn('--if-exists', self.drops()[0])
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertEqual(len(self.drops()), 1)

    def test_unobserved_case_cannot_be_released(self):
        with self.environment.owned() as active:
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())

    def test_a_witness_path_cannot_be_reused_for_another_database(self):
        with self.environment.owned() as active:
            self.observe(active)
            with self.assertRaises(FileExistsError):
                active.observe_database('native_selection', 'native_selection_state.sql',
                                        'item-mutation-observation.txt')
            with self.assertRaises(ValueError):
                active.retire_database('native_selection')
            self.assertFalse(self.drops())

    def test_source_templates_restored_and_pending_auth_are_never_retirement_targets(self):
        with self.environment.owned() as active:
            for name in ('postgres', 'template0', 'template1', 'kehila_native_restored',
                         'native_auth_owned', 'production'):
                with self.subTest(name=name), self.assertRaises(ValueError):
                    active.retire_database(name)
            self.assertFalse(self.drops())

    def test_an_unrelated_or_path_escaped_observer_cannot_issue_eligibility(self):
        with self.environment.owned() as active:
            for observer, artifact in (
                    ('native_selection_state.sql', 'item-mutation-observation.txt'),
                    ('../native_item_mutation_state.sql', 'item-mutation-observation.txt'),
                    ('native_item_mutation_state.sql', '../outside-observation.txt')):
                with self.subTest(observer=observer, artifact=artifact), self.assertRaises(ValueError):
                    active.observe_database('native_item_mutation', observer, artifact)
            self.assertFalse(self.drops())

    def test_failed_observer_does_not_create_eligibility(self):
        with self.environment.owned() as active:
            self.executor.fail_observer = '/tmp/native_item_mutation_state.sql'
            with self.assertRaises(RuntimeError):
                self.observe(active)
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())

    def test_missing_or_changed_witness_refuses_retirement(self):
        with self.environment.owned() as active:
            self.observe(active)
            witness = active.folder / 'item-mutation-observation.txt'
            witness.write_text('different witness', encoding='utf-8')
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            witness.unlink()
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())

    def test_changed_observer_source_or_transferred_bytes_refuse_observation_and_release(self):
        with self.environment.owned() as active:
            path = self.source / 'tests/integration/native_item_mutation_state.sql'
            original = path.read_bytes()
            path.write_bytes(original + b'\n-- changed\n')
            with self.assertRaises(ValueError):
                self.observe(active)
            path.write_bytes(original)
            self.observe(active)
            path.write_bytes(original + b'\n-- changed again\n')
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())

    def test_replaced_database_identity_owner_or_flags_are_refused(self):
        with self.environment.owned() as active:
            self.observe(active)
            row = self.executor.databases['native_item_mutation']
            for key, value in (('oid', 99999), ('owner', 'different_owner'),
                               ('allow', 0), ('template', 1)):
                before = row[key]
                row[key] = value
                with self.subTest(key=key), self.assertRaises(ValueError):
                    active.retire_database('native_item_mutation')
                row[key] = before
            self.assertFalse(self.drops())

    def test_live_connections_prevent_drop_without_force_or_termination(self):
        with self.environment.owned() as active:
            self.observe(active)
            self.executor.databases['native_item_mutation']['connections'] = 1
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())
            self.assertFalse(any('pg_terminate_backend' in str(args) for args in self.executor.events))

    def test_an_unexpectedly_missing_database_is_not_suppressed(self):
        with self.environment.owned() as active:
            self.observe(active)
            del self.executor.databases['native_item_mutation']
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())

    def test_foreign_container_or_changed_resource_profile_prevents_retirement(self):
        with self.environment.owned() as active:
            self.observe(active)
            for flag in ('foreign', 'profile_mismatch'):
                setattr(self.executor, flag, True)
                try:
                    with self.subTest(flag=flag), self.assertRaises(ValueError):
                        active.retire_database('native_item_mutation')
                finally:
                    setattr(self.executor, flag, False)
            self.assertFalse(self.drops())

    def test_drop_failure_keeps_witness_and_stops_another_retirement(self):
        with self.environment.owned() as active:
            self.observe(active)
            self.observe(active, 'native_selection')
            self.executor.fail_drop = True
            with self.assertRaises(RuntimeError):
                active.retire_database('native_item_mutation')
            self.assertTrue((active.folder / 'item-mutation-observation.txt').is_file())
            self.assertIn('native_item_mutation', self.executor.databases)
            with self.assertRaises(ValueError):
                active.retire_database('native_selection')
            self.assertEqual(len(self.drops()), 1)

    def test_an_external_dependent_owner_cannot_be_bypassed_by_retirement(self):
        with self.environment.owned() as active:
            self.observe(active)
            active.release_dependents = lambda *_: None
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertFalse(self.drops())

    def test_successful_drop_command_cannot_replace_confirmed_absence(self):
        with self.environment.owned() as active:
            self.observe(active)
            self.executor.keep_after_drop = True
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            command = active.folder / 'retirement-native_item_mutation-command.json'
            confirmed = active.folder / 'retirement-native_item_mutation-confirmed.json'
            self.assertEqual(json.loads(command.read_text())['state'], 'drop_returned_success')
            self.assertFalse(confirmed.exists())
            with self.assertRaises(ValueError):
                active.retire_database('native_item_mutation')
            self.assertEqual(len(self.drops()), 1)

    def test_partial_retirement_preserves_confirmed_history_and_pending_copies(self):
        with self.environment.owned() as active:
            self.observe(active)
            self.observe(active, 'native_selection')
            pending = dict(self.executor.databases['native_auth_owned'])
            active.retire_database('native_item_mutation')
            confirmed = active.folder / 'retirement-native_item_mutation-confirmed.json'
            original = confirmed.read_bytes()
            self.executor.fail_drop = True
            with self.assertRaises(RuntimeError):
                active.retire_database('native_selection')
            self.assertNotIn('native_item_mutation', self.executor.databases)
            self.assertIn('native_selection', self.executor.databases)
            self.assertEqual(self.executor.databases['native_auth_owned'], pending)
            self.assertEqual(confirmed.read_bytes(), original)
            with self.assertRaises(ValueError):
                active.retire_database('native_selection')
            self.assertEqual(len(self.drops()), 2)


if __name__ == '__main__':
    unittest.main()
