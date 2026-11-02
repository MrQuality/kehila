"""Offline safety and false-success checks; these do not execute PostgreSQL."""
import importlib.util
import io
from pathlib import Path
import sys
import subprocess
import unittest
from unittest.mock import Mock, patch


spec = importlib.util.spec_from_file_location(
    'sp005_run', Path(__file__).resolve().parents[1] / 'experiments/SP-005/run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RunnerBoundaryTests(unittest.TestCase):
    def test_expired_work_budget_prevents_process_execution(self):
        with patch.object(runner.time, 'monotonic', side_effect=[10, 20]), \
                patch.object(runner.native.subprocess, 'run') as process:
            database = runner.MappingCheck(['podman'], 'fixture', seconds=1)
            with self.assertRaisesRegex(ValueError, 'budget expired'):
                database.command('inspect', 'fixture')
        process.assert_not_called()

    def test_process_timeout_is_capped_by_remaining_budget(self):
        completed = subprocess.CompletedProcess([], 0, '', '')
        with patch.object(runner.time, 'monotonic', side_effect=[10, 11]), \
                patch.object(runner.native.subprocess, 'run', return_value=completed) as process:
            database = runner.MappingCheck(['podman'], 'fixture', seconds=5)
            database.command('inspect', 'fixture')
        self.assertEqual(process.call_args.kwargs['timeout'], 4)

    def test_invalid_budget_is_rejected(self):
        for seconds in (0, -1, float('nan'), float('inf'), 601):
            with self.subTest(seconds=seconds), self.assertRaises(ValueError):
                runner.MappingCheck(['podman'], 'fixture', seconds=seconds)

    def test_invalid_target_is_rejected_before_resources_or_database(self):
        for value in ('../other', 'a;whoami', '-option', 'x' * 129):
            with self.subTest(value=value), \
                    patch.object(sys, 'argv', ['run.py', '--disposable-container', value]), \
                    patch.object(runner.native, 'preflight') as preflight, \
                    patch.object(runner.native, 'DatabaseCheck') as database:
                with self.assertRaises((ValueError, SystemExit)):
                    runner.main()
                preflight.assert_not_called()
                database.assert_not_called()

    def test_invalid_connection_is_rejected_before_resources_or_database(self):
        with patch.object(sys, 'argv', ['run.py', '--disposable-container', 'fixture',
                                       '--podman-connection', '../other']), \
                patch.object(runner.native, 'preflight') as preflight, \
                patch.object(runner.native, 'DatabaseCheck') as database:
            with self.assertRaises(ValueError):
                runner.main()
            preflight.assert_not_called()
            database.assert_not_called()

    def test_baseline_failure_prevents_overlay_and_success_output(self):
        database = Mock()
        database.prepare_structure.side_effect = ValueError('baseline failed')
        with patch('sys.stdout', new_callable=io.StringIO) as output:
            with self.assertRaisesRegex(ValueError, 'baseline failed'):
                runner.run(database)
        database.sql.assert_not_called()
        self.assertEqual(output.getvalue(), '')

    def test_missing_or_duplicate_oracle_marker_cannot_pass(self):
        for output in ('', 'SP005_SERVING_PASS\nSP005_SERVING_PASS\n'):
            with self.subTest(output=output), self.assertRaisesRegex(
                    ValueError, 'oracle marker'):
                runner.require_marker(output, 'SP005_SERVING_PASS')

    def test_native_case_failure_cannot_return_success(self):
        database = Mock()
        database.sql.side_effect = [None, 'SP005_FIXTURE_PASS\n',
                                    ValueError('serving oracle failed')]
        with self.assertRaisesRegex(ValueError, 'serving oracle failed'):
            runner.run(database)
        self.assertEqual(database.sql.call_count, 3)

    def test_structural_success_requires_all_oracles_and_restricts_scope(self):
        database = Mock()
        database.sql.side_effect = [None, 'SP005_FIXTURE_PASS\n',
                                    'SP005_SERVING_PASS\n', 'SP005_OWNER_PASS\n', '']
        result = runner.run(database)
        database.prepare_structure.assert_called_once_with()
        self.assertEqual(result['scope'], 'SP-005 physical mapping structural subset')
        self.assertIn('atomic application project creation with mapping', result['not_qualified'])
        self.assertEqual(database.sql.call_args.kwargs['user'], 'foundation_untrusted')
        self.assertEqual(database.sql.call_args.kwargs['expected_state'], '42501')

    def test_preflight_failure_prevents_database_and_result(self):
        with patch.object(sys, 'argv', ['run.py', '--disposable-container', 'fixture']), \
                patch.object(runner.native, 'preflight', side_effect=ValueError('memory')), \
                patch.object(runner.native, 'DatabaseCheck') as database, \
                patch('sys.stdout', new_callable=io.StringIO) as output:
            with self.assertRaisesRegex(ValueError, 'memory'):
                runner.main()
        database.assert_not_called()
        self.assertEqual(output.getvalue(), '')
