"""Bounds and failure cases for child-owned ephemeral listener discovery."""
import importlib.util
from pathlib import Path
import sys
import tempfile
from unittest.mock import Mock
from matrix_fixtures import IsolatedEnvironmentTests

INTEGRATION = Path(__file__).resolve().parent / 'integration'
sys.path.insert(0, str(INTEGRATION))
spec = importlib.util.spec_from_file_location('task_path', INTEGRATION / 'task_path.py')
task_path = importlib.util.module_from_spec(spec)
spec.loader.exec_module(task_path)


class BoundAddressTests(IsolatedEnvironmentTests):
    def discover(self, output, returncode=None, timeout=1):
        with tempfile.TemporaryDirectory() as directory:
            log = Path(directory, 'child.log')
            log.write_bytes(output)
            return task_path.bound_base(log, Mock(poll=Mock(return_value=returncode)), timeout)

    def test_complete_loopback_address_with_lf_or_crlf(self):
        for ending in (b'\n', b'\r\n'):
            self.assertEqual(self.discover(b'KEHILA_BOUND_ADDR=127.0.0.1:43210' + ending),
                             'http://127.0.0.1:43210')

    def test_zero_out_of_range_non_loopback_and_malformed_address_rejected(self):
        for address in (b'127.0.0.1:0', b'127.0.0.1:65536', b'0.0.0.0:43210', b'garbage'):
            with self.subTest(address=address), self.assertRaisesRegex(AssertionError, 'invalid bound'):
                self.discover(b'KEHILA_BOUND_ADDR=' + address + b'\n')

    def test_duplicate_reports_are_rejected(self):
        with self.assertRaisesRegex(AssertionError, 'multiple bound'):
            self.discover(b'KEHILA_BOUND_ADDR=127.0.0.1:43210\n' * 2)

    def test_exited_child_reports_its_diagnostic(self):
        with self.assertRaisesRegex(AssertionError, 'address already in use'):
            self.discover(b'address already in use\n', returncode=1)

    def test_exited_child_with_oversized_output_retains_exit_status_and_bounded_diagnostic(self):
        output = b'PANIC: startup failure\n' + b'x' * 100000
        with self.assertRaisesRegex(AssertionError, 'Child exited.*7') as caught:
            self.discover(output, returncode=7)
        diagnostic = str(caught.exception)
        self.assertIn('PANIC: startup failure', diagnostic)
        self.assertIn('truncated', diagnostic)
        self.assertLess(len(diagnostic), 8400)

    def test_output_exact_limit_and_overflow(self):
        marker = b'KEHILA_BOUND_ADDR=127.0.0.1:43210\n'
        output = b'x' * (8192 - len(marker) - 1) + b'\n' + marker
        self.assertEqual(self.discover(output), 'http://127.0.0.1:43210')
        with self.assertRaisesRegex(AssertionError, 'exceeds 8 KiB'):
            self.discover(b'x' * 8193)

    def test_partial_line_cannot_satisfy_discovery(self):
        with self.assertRaisesRegex(AssertionError, 'deadline exceeded'):
            self.discover(b'KEHILA_BOUND_ADDR=127.0.0.1:43210', timeout=0.01)
