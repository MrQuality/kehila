"""Failure propagation and prerequisite ordering for the opt-in A05 runner."""
import importlib.util
import io
from pathlib import Path
import runpy
import sys
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location(
    'sp004_run', Path(__file__).resolve().parents[1] / 'experiments/SP-004/run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RunnerBoundaryTests(unittest.TestCase):
    def test_unsafe_connection_is_rejected_before_preflight_or_database_setup(self):
        for connection in ('', '../other', '-flag', 'ssh://remote', 'bad name', 'x' * 129):
            with self.subTest(connection=connection), \
                    patch.object(sys, 'argv', ['run.py', '--disposable-container', 'fixture',
                                              '--podman-connection=' + connection]), \
                    patch.object(runner.native, 'preflight') as preflight, \
                    patch.object(runner.native, 'DatabaseCheck') as database:
                with self.assertRaisesRegex(ValueError, 'invalid container or connection'):
                    runner.main()
                preflight.assert_not_called()
                database.assert_not_called()

    def test_valid_connection_reaches_the_database_without_using_default_routing(self):
        with patch.object(sys, 'argv', ['run.py', '--disposable-container', 'fixture',
                                      '--podman-connection', 'fixture-root']), \
                patch.object(runner.native, 'preflight'), \
                patch.object(runner.native, 'DatabaseCheck') as database, \
                patch.object(runner, 'run', return_value={'outcome': 'fixture'}) as run, \
                patch('sys.stdout', new_callable=io.StringIO):
            runner.main()
        database.assert_called_once_with(['podman', '--connection', 'fixture-root'], 'fixture')
        run.assert_called_once_with(database.return_value)

    def test_cli_reports_cleanup_causes_and_returns_130_without_interrupt_traceback(self):
        primary = ValueError('baseline failed')
        primary.__cause__ = OSError('snapshot cleanup failed')
        for failure, expected_code in ((primary, 3), (KeyboardInterrupt(), 130)):
            with self.subTest(failure=type(failure).__name__), \
                    patch.object(sys, 'argv', ['run.py', '--disposable-container', 'fixture']), \
                    patch.object(runner.native, 'preflight'), \
                    patch.object(runner.native, 'DatabaseCheck') as database, \
                    patch('sys.stderr', new_callable=io.StringIO) as diagnostic:
                database.return_value.run.side_effect = failure
                try:
                    runpy.run_path(str(Path(runner.__file__)), run_name='__main__')
                except SystemExit as error:
                    self.assertEqual(error.code, expected_code)
                except KeyboardInterrupt:
                    self.fail('CLI did not handle interruption')
                else:
                    self.fail('CLI did not return an exit status')
                self.assertNotIn('Traceback', diagnostic.getvalue())
                if failure is primary:
                    self.assertIn('ERR_SP004_CHECK: baseline failed', diagnostic.getvalue())
                    self.assertIn('CAUSE: snapshot cleanup failed', diagnostic.getvalue())

    def test_failed_native_baseline_prevents_overlay(self):
        database = Mock()
        database.run.side_effect = ValueError('native baseline failed')
        with self.assertRaisesRegex(ValueError, 'native baseline failed'):
            runner.run(database)
        database.sql.assert_not_called()

    def test_all_null_inputs_authority_denials_and_missing_grant_are_exercised(self):
        database = Mock()
        runner.run(database)
        rejections = [call.kwargs for call in database.sql.call_args_list
                      if call.kwargs.get('expected_state')]
        self.assertEqual(sum(case['expected_state'] == '22004' for case in rejections), 6)
        self.assertTrue(any(case['expected_state'] == '23503' and
                            case['expected_message'] == 'grant does not exist' for case in rejections))
        self.assertEqual(sum(case.get('expected_message') == 'management authority denied'
                             for case in rejections), 4)

    def test_missing_baseline_guard_prevents_overlay(self):
        database = Mock()
        database.sql.side_effect = ValueError('expected creation-only rejection absent')
        with self.assertRaisesRegex(ValueError, 'creation-only rejection absent'):
            runner.run(database)
        self.assertEqual(database.sql.call_count, 1)
        self.assertEqual(database.sql.call_args.kwargs['expected_state'], '23514')

    def test_failed_overlay_prevents_role_and_management_cases(self):
        database = Mock()
        database.sql.side_effect = [None, ValueError('overlay failed')]
        with self.assertRaisesRegex(ValueError, 'overlay failed'):
            runner.run(database)
        self.assertEqual(database.sql.call_count, 2)

    def test_native_rejection_or_assertion_failure_cannot_produce_success(self):
        for diagnostic in ('permission denied for function change_grant',
                           'permission denied for table project_grants',
                           'management writes require the restricted helper',
                           'stale grant version', 'grant change is a no-op',
                           'management rollback left state',
                           'management attribution or payload mismatch',
                           'corruption probe changed committed state',
                           'permission denied for table project_grant_events',
                           'all management inputs are required',
                           'management authority denied', 'grant does not exist',
                           'management rejection changed committed state',
                           'distinct management rollback left state',
                           'distinct management attribution or payload mismatch',
                           'creation history is append-only'):
            with self.subTest(diagnostic=diagnostic):
                database = Mock()
                reached = False
                calls = 0

                def fail(*args, **kwargs):
                    nonlocal reached, calls
                    calls += 1
                    # The overlay DDL contains rejection messages but does not execute those cases.
                    if calls > 2 and (diagnostic in args[0] or kwargs.get('expected_message') == diagnostic):
                        reached = True
                        raise ValueError('native case failed')

                database.sql.side_effect = fail
                with self.assertRaisesRegex(ValueError, 'native case failed'):
                    runner.run(database)
                self.assertTrue(reached)

    def test_unsafe_container_argument_is_rejected_before_preflight_or_io(self):
        with patch.object(sys, 'argv', ['run.py', '--disposable-container', '../real-db']), \
             patch.object(runner.native, 'preflight') as preflight, \
             patch.object(runner.native, 'DatabaseCheck') as database:
            with self.assertRaisesRegex(ValueError, 'invalid container'):
                runner.main()
        preflight.assert_not_called()
        database.assert_not_called()

    def test_failed_resource_preflight_prevents_native_setup(self):
        with patch.object(sys, 'argv', ['run.py', '--disposable-container', 'disposable']), \
             patch.object(runner.native, 'preflight', side_effect=ValueError('memory low')), \
             patch.object(runner.native, 'DatabaseCheck') as database:
            with self.assertRaisesRegex(ValueError, 'memory low'):
                runner.main()
        database.assert_not_called()


if __name__ == '__main__':
    unittest.main()
