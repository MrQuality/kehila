"""Control-plane contracts for the opt-in SQL structure verifier; no service calls."""
import importlib.util
import json
from pathlib import Path
import sys
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
spec = importlib.util.spec_from_file_location(
    'check_work_item_storage', Path(__file__).resolve().parents[1] / 'scripts/check_work_item_storage.py')
check = importlib.util.module_from_spec(spec)
spec.loader.exec_module(check)


class StructureControllerTests(unittest.TestCase):
    identity = 'a' * 64

    def setUp(self):
        control_patch = patch.object(check, 'check_core_wait')
        self.core_wait = control_patch.start()
        self.addCleanup(control_patch.stop)

    def database(self):
        database = Mock(container='disposable')
        database.command.return_value = SimpleNamespace(
            returncode=0, stdout=json.dumps([{'Id': self.identity}]), stderr='')
        return database

    def test_inspection_failure_prevents_all_database_work(self):
        database = self.database()
        database.command.return_value.returncode = 1
        with self.assertRaises(ValueError):
            check.check(database)
        database.sql.assert_not_called()

    def test_missing_or_ambiguous_identity_prevents_database_work(self):
        for objects in ([{}], [], [{'Id': self.identity}, {'Id': 'b' * 64}]):
            with self.subTest(objects=objects):
                database = self.database()
                database.command.return_value.stdout = json.dumps(objects)
                with patch.object(check, 'validate_container'):
                    with self.assertRaises(ValueError):
                        check.check(database)
                database.sql.assert_not_called()

    def test_unsafe_container_prevents_all_database_work(self):
        database = self.database()
        with patch.object(check, 'validate_container', side_effect=ValueError('unsafe')):
            with self.assertRaisesRegex(ValueError, 'unsafe'):
                check.check(database)
        database.sql.assert_not_called()

    def test_existing_database_rejection_precedes_copy_and_install(self):
        database = self.database()
        database.sql.side_effect = ValueError('not fresh')
        with patch.object(check, 'validate_container'):
            with self.assertRaisesRegex(ValueError, 'not fresh'):
                check.check(database)
        self.assertEqual(database.command.call_count, 1)
        database.sql.assert_called_once_with(check.FRESH)

    def test_failed_copy_cannot_install_or_run_fixture(self):
        database = self.database()
        database.command.side_effect = [
            SimpleNamespace(returncode=0, stdout=json.dumps([{'Id': self.identity}]), stderr=''),
            SimpleNamespace(returncode=1, stdout='', stderr='copy failed')]
        with patch.object(check, 'validate_container'):
            with self.assertRaisesRegex(ValueError, 'copy failed'):
                check.check(database)
        database.sql.assert_called_once_with(check.FRESH)

    def test_wrong_installed_version_cannot_run_fixture_or_success_observer(self):
        database = self.database()
        database.sql.side_effect = [None, None, None, ValueError('overlay failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'overlay failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 4)
        output.assert_not_called()

    def test_fixture_failure_cannot_report_success_or_run_observer(self):
        database = self.database()
        database.sql.side_effect = [None] * 7 + [ValueError('fixture failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'fixture failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 8)
        output.assert_not_called()

    def test_observer_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 8 + [ValueError('observer failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'observer failed'):
                check.check(database)
        output.assert_not_called()

    def test_private_capability_oracle_failure_prevents_fixture(self):
        database = self.database()
        database.sql.side_effect = [None] * 4 + [ValueError('denial missing')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'denial missing'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 5)
        output.assert_not_called()

    def test_history_fixture_failure_cannot_run_final_observer(self):
        database = self.database()
        database.sql.side_effect = [None] * 9 + [ValueError('history failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'history failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 10)
        output.assert_not_called()

    def test_history_observer_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 10 + [ValueError('history observer failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'history observer failed'):
                check.check(database)
        output.assert_not_called()

    def test_success_follows_installed_profile_and_separate_observation(self):
        database = self.database()
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            check.check(database)
        statements = database.sql.call_args_list
        self.assertEqual(statements[1].kwargs, {'file': check.DESTINATION + '/install_product.sql'})
        self.assertEqual(statements[3].args, (check.INSTALLED,))
        for denial in statements[4:7]:
            self.assertEqual(denial.kwargs['expected_state'], '42501')
            self.assertIn('permission denied', denial.kwargs['expected_message'])
        self.assertEqual([call.kwargs for call in statements[-13:]], [
            {'file': '/opt/kehila-work-item-storage.sql'},
            {'file': '/opt/kehila-work-item-storage-state.sql'},
            {'file': '/opt/kehila-work-item-history.sql'},
            {'file': '/opt/kehila-work-item-history-state.sql'},
            {'file': '/opt/kehila-work-item-authority.sql'},
            {'file': '/opt/kehila-work-item-pending.sql'},
            {'file': '/opt/kehila-work-item-pending-state.sql'},
            {'file': '/opt/kehila-work-item-discovery.sql'},
            {'file': '/opt/kehila-work-item-discovery-state.sql'},
            {'file': '/opt/kehila-work-item-reconciliation.sql'},
            {'file': '/opt/kehila-work-item-reconciliation-state.sql'},
            {'file': '/opt/kehila-work-item-save.sql'},
            {'file': '/opt/kehila-work-item-save-state.sql'}])
        self.assertEqual(database.container, self.identity)
        self.assertTrue(any(call.args[-1] == self.identity + ':/opt/kehila-work-item-storage-state.sql'
                            for call in database.command.call_args_list))
        output.assert_called_once()
        self.core_wait.assert_called_once_with(database)

    def test_authority_fixture_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 11 + [ValueError('authority failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'authority failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 12)
        output.assert_not_called()

    def test_pending_fixture_failure_cannot_run_its_observer(self):
        database = self.database()
        database.sql.side_effect = [None] * 12 + [ValueError('pending failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'pending failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 13)
        output.assert_not_called()

    def test_pending_observer_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 13 + [ValueError('pending observer failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'pending observer failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 14)
        output.assert_not_called()

    def test_discovery_fixture_failure_cannot_run_its_observer(self):
        database = self.database()
        database.sql.side_effect = [None] * 14 + [ValueError('discovery failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'discovery failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 15)
        output.assert_not_called()

    def test_discovery_observer_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 15 + [ValueError('discovery observer failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'discovery observer failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 16)
        output.assert_not_called()

    def test_reconciliation_fixture_failure_cannot_run_its_observer(self):
        database = self.database()
        database.sql.side_effect = [None] * 16 + [ValueError('reconciliation failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'reconciliation failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 17)
        output.assert_not_called()

    def test_reconciliation_observer_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 17 + [ValueError('reconciliation observer failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'reconciliation observer failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 18)
        output.assert_not_called()

    def test_save_fixture_failure_cannot_run_its_observer(self):
        database = self.database()
        database.sql.side_effect = [None] * 18 + [ValueError('save failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'save failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 19)
        output.assert_not_called()

    def test_save_observer_failure_cannot_report_success(self):
        database = self.database()
        database.sql.side_effect = [None] * 19 + [ValueError('save observer failed')]
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'save observer failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 20)
        output.assert_not_called()

    def test_core_wait_failure_prevents_final_observer_and_success(self):
        database = self.database()
        self.core_wait.side_effect = ValueError('core wait failed')
        with patch.object(check, 'validate_container'), patch('builtins.print') as output:
            with self.assertRaisesRegex(ValueError, 'core wait failed'):
                check.check(database)
        self.assertEqual(database.sql.call_count, 19)
        self.assertEqual(database.sql.call_args_list[-1].kwargs, {'file': '/opt/kehila-work-item-save.sql'})
        output.assert_not_called()

    def test_core_wait_follows_save_and_precedes_its_observer(self):
        database = self.database()
        self.core_wait.side_effect = lambda _: self.assertEqual(database.sql.call_count, 19)
        with patch.object(check, 'validate_container'), patch('builtins.print'):
            check.check(database)
        self.assertEqual(database.sql.call_args_list[-1].kwargs, {'file': '/opt/kehila-work-item-save-state.sql'})


class CoreWaitCommandTests(unittest.TestCase):
    def test_failed_control_cannot_satisfy_marker(self):
        database = Mock(container='a' * 64)
        database.command.return_value = SimpleNamespace(returncode=41, stdout='PASS: ignored', stderr='expected missing fence')
        with self.assertRaisesRegex(ValueError, 'expected missing fence'):
            check.check_core_wait(database)

    def test_success_requires_exact_observation_marker(self):
        database = Mock(container='a' * 64)
        database.command.return_value = SimpleNamespace(returncode=0, stdout='', stderr='')
        with self.assertRaisesRegex(ValueError, 'observation absent'):
            check.check_core_wait(database)
        database.command.return_value.stdout = 'PASS: predecessor core wait precedes item acquisition and leaves no accepted work\n'
        check.check_core_wait(database)
        self.assertEqual(database.command.call_args.args[:3], ('exec', 'a' * 64, 'timeout'))
        self.assertEqual(database.command.call_args.kwargs, {'timeout': 35})


if __name__ == '__main__':
    unittest.main()
