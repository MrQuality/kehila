"""Failure propagation and prerequisite ordering for the opt-in A05 runner."""
import importlib.util
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location(
    'sp004_run', Path(__file__).resolve().parents[1] / 'experiments/SP-004/run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RunnerBoundaryTests(unittest.TestCase):
    def test_failed_native_baseline_prevents_overlay(self):
        database = Mock()
        database.run.side_effect = ValueError('native baseline failed')
        with self.assertRaisesRegex(ValueError, 'native baseline failed'):
            runner.run(database)
        database.sql.assert_not_called()

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
        for diagnostic in ('permission denied for function change_grant', 'stale grant version',
                           'management attribution or payload mismatch',
                           'corruption probe changed committed state',
                           'creation history is append-only'):
            with self.subTest(diagnostic=diagnostic):
                database = Mock()
                reached = False

                def fail(*args, **kwargs):
                    nonlocal reached
                    if diagnostic in args[0] or kwargs.get('expected_message') == diagnostic:
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
