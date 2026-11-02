"""Offline checks for failure diagnosis without live workers or services."""
import importlib.util
import io
from pathlib import Path
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'tests/integration'))
SPEC = importlib.util.spec_from_file_location('task_path_diagnostics', ROOT / 'tests/integration/task_path.py')
TASK = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(TASK)


class DiagnosticsTests(unittest.TestCase):
    def test_case_preserves_reset_and_does_not_convert_it_to_success(self):
        reset = ConnectionResetError('reset')
        with self.assertRaisesRegex(RuntimeError, 'stalled body response') as caught:
            with TASK.boundary_case('stalled body response'):
                raise reset
        self.assertIs(caught.exception.__cause__, reset)

    def test_wrong_status_assertion_retains_case(self):
        with self.assertRaisesRegex(RuntimeError, 'rejected POST') as caught:
            with TASK.boundary_case('rejected POST'):
                raise AssertionError('expected 403')
        self.assertIsInstance(caught.exception.__cause__, AssertionError)

    def test_log_tail_is_bounded(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'process-0.log').write_bytes(b'old-prefix' + b'x' * 8192)
            with patch('sys.stderr', new_callable=io.StringIO) as output:
                TASK.report_process_logs(folder)
        self.assertNotIn('old-prefix', output.getvalue())
        self.assertIn('x' * 8192, output.getvalue())

    def test_unreadable_diagnostics_do_not_raise(self):
        with patch.object(TASK.Path, 'glob', side_effect=PermissionError('denied')), \
                patch('sys.stderr', new_callable=io.StringIO) as output:
            TASK.report_process_logs('unused')
        self.assertIn('DIAGNOSTIC UNAVAILABLE', output.getvalue())

    def test_broken_diagnostic_stream_does_not_raise(self):
        with tempfile.TemporaryDirectory() as folder:
            (Path(folder) / 'process-0.log').write_text('diagnostic')
            with patch('builtins.print', side_effect=OSError('closed stream')):
                TASK.report_process_logs(folder)

    def test_exercise_preserves_primary_when_reporter_fails(self):
        primary = RuntimeError('installation failed')
        diagnostic = ValueError('closed diagnostic stream')
        with tempfile.TemporaryDirectory() as folder:
            extension = '.exe' if TASK.os.name == 'nt' else ''
            for name in ('task_worker', 'task-api'):
                (Path(folder) / (name + extension)).touch()
            with patch.object(TASK.subprocess, 'run', side_effect=[primary, None]), \
                    patch.object(TASK, 'report_process_logs', side_effect=diagnostic):
                with self.assertRaises(RuntimeError) as caught:
                    TASK.exercise(Path(folder))
        self.assertIs(caught.exception, primary)
        self.assertEqual(primary._cleanup_failures,
                         (('task process diagnostics', diagnostic),))
