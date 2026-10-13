"""Failure and framing contracts for the production-codec/native test bridge."""
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
import unittest
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    'postgresql_snapshots', Path(__file__).resolve().parents[1] / 'scripts/postgresql_snapshots.py')
bridge = importlib.util.module_from_spec(spec)
spec.loader.exec_module(bridge)


class ProcessTests(unittest.TestCase):
    def test_bounded_input_output_and_nonzero_exit_are_preserved(self):
        result = bridge.bounded([sys.executable, '-c',
            'import sys;sys.stdout.buffer.write(sys.stdin.buffer.read());sys.exit(7)'], b'exact')
        self.assertEqual(result.stdout, b'exact')
        self.assertEqual(result.returncode, 7)

    def test_either_pipe_overflow_rejects_instead_of_accepting_truncation(self):
        for stream, cap in [('stdout', 32), ('stderr', 65536)]:
            with self.subTest(stream=stream), self.assertRaisesRegex(ValueError, 'overflow'):
                bridge.bounded([sys.executable, '-c',
                    f'import sys;sys.{stream}.buffer.write(b"x"*{cap + 1})'], limit=32)

    def test_timeout_terminates_the_owned_process(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            bridge.bounded([sys.executable, '-c', 'import time;time.sleep(30)'], timeout=0.1)

    def test_timeout_also_terminates_child_holding_output_pipes(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            bridge.bounded([sys.executable, '-c',
                'import subprocess,sys,time;subprocess.Popen([sys.executable,"-c",'
                '"import time;time.sleep(30)"]);time.sleep(30)'], timeout=0.3)


class SqlBridgeTests(unittest.TestCase):
    class Database:
        podman = ['podman', '--connection', 'test']
        container = 'explicit-disposable'

    def checker(self):
        return bridge.SnapshotCheck(self.Database(), Path('test-helper'))

    def test_raw_json_and_noncanonical_hex_cannot_be_interpolated_into_sql(self):
        for value in ["' ; DROP SCHEMA kehila; --", '{}', '00ff\n', '0', 'AA', None]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                bridge.sql_text(value)

    def test_sql_frame_requires_exact_fields_and_one_row(self):
        valid = {'project': '00', 'configuration': '00', 'revision': '1',
                 'project_id': 'fixture', 'codec_version': 1}
        with patch.object(bridge, 'bounded', return_value=subprocess.CompletedProcess([], 0,
                json.dumps(valid).encode(), b'')):
            self.assertEqual(self.checker().sql('SELECT fixture'), valid)
        for stdout in [b'{}', b'{}\n{}', b'null', json.dumps({**valid, 'extra': 1}).encode()]:
            with self.subTest(stdout=stdout), patch.object(bridge, 'bounded',
                return_value=subprocess.CompletedProcess([], 0, stdout, b'')), self.assertRaises(ValueError):
                self.checker().sql('SELECT fixture')

    def test_sql_negative_requires_script_exit_and_actual_error_state(self):
        for code, stderr in [(0, b'ERROR:  23514: bad'), (125, b'ERROR:  23514: bad'),
                             (3, b'ERROR:  23503: bad'), (3, b'CONTEXT: 23514')]:
            with self.subTest(code=code, stderr=stderr), patch.object(bridge, 'bounded',
                return_value=subprocess.CompletedProcess([], code, b'', stderr)), self.assertRaises(ValueError):
                self.checker().sql('SELECT bad', state='23514')
        with patch.object(bridge, 'bounded', return_value=subprocess.CompletedProcess([], 3,
                b'', b'ERROR:  23514: bad')):
            self.checker().sql('SELECT bad', state='23514')

    def test_oracle_rejection_cannot_be_satisfied_by_a_crash_or_unrelated_error(self):
        for code, stdout, stderr in [(0, b'', b''), (1, b'', b'ERR_SNAPSHOT_ORACLE:'),
                (3, b'partial', b'ERR_SNAPSHOT_ORACLE:'), (3, b'', b'unrelated')]:
            with self.subTest(code=code), patch.object(bridge, 'bounded',
                return_value=subprocess.CompletedProcess([], code, stdout, stderr)), self.assertRaises(ValueError):
                self.checker().oracle('verify', 'seed', {}, reject=True)


if __name__ == '__main__':
    unittest.main()
