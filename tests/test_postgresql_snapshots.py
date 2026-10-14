"""Failure and framing contracts for the production-codec/native test bridge."""
import importlib.util
import json
import os
import subprocess
import sys
import time
import tempfile
from collections import Counter
from pathlib import Path
import unittest
from unittest.mock import patch, Mock

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

    def test_pipe_close_failure_cannot_replace_the_original_reader_failure(self):
        popen = bridge.subprocess.Popen
        class BrokenPipe:
            def __init__(self, stream): self.stream = stream
            def read(self, count): raise OSError('read failed')
            def close(self):
                self.stream.close()
                raise OSError('close failed')
        def spawn(*args, **kwargs):
            process = popen(*args, **kwargs)
            process.stdout = BrokenPipe(process.stdout)
            return process
        with patch.object(bridge.subprocess, 'Popen', side_effect=spawn):
            with self.assertRaisesRegex(ValueError, 'pipe read failed'):
                bridge.bounded([sys.executable, '-c', 'import time;time.sleep(3)'], timeout=1)

    def test_timeout_terminates_the_owned_process(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            bridge.bounded([sys.executable, '-c', 'import time;time.sleep(30)'], timeout=0.1)

    def test_timeout_also_terminates_child_holding_output_pipes(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            bridge.bounded([sys.executable, '-c',
                'import subprocess,sys,time;subprocess.Popen([sys.executable,"-c",'
                '"import time;time.sleep(30)"]);time.sleep(30)'], timeout=0.3)

    def test_exited_parent_cannot_leave_descendant_pipes_past_deadline(self):
        started = time.monotonic()
        command = [sys.executable, '-c',
            'import subprocess,sys;child=subprocess.Popen([sys.executable,"-c",'
            '"import time;time.sleep(3)"]);print(child.pid,flush=True)']
        if os.name == 'nt':
            result = bridge.bounded(command, timeout=1)
            self.assertEqual(result.returncode, 0)
            # Successful early cleanup is valid only if the descendant is dead.
            import ctypes
            from ctypes import wintypes
            kernel = ctypes.WinDLL('kernel32', use_last_error=True)
            kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
            kernel.OpenProcess.restype = wintypes.HANDLE
            kernel.WaitForSingleObject.argtypes = [wintypes.HANDLE, wintypes.DWORD]
            kernel.CloseHandle.argtypes = [wintypes.HANDLE]
            child = kernel.OpenProcess(0x100000, False, int(result.stdout))
            if child:
                try:
                    # Job termination is asynchronous. Observe actual exit within
                    # the original execution deadline, without an extra sleep/budget.
                    remaining_ms = max(0, int((started + 1 - time.monotonic()) * 1000))
                    self.assertEqual(kernel.WaitForSingleObject(child, remaining_ms), 0,
                                     'descendant outlived the original execution deadline')
                finally:
                    kernel.CloseHandle(child)
            else:
                self.assertEqual(ctypes.get_last_error(), 87, 'cannot inspect owned descendant')
        else:
            with self.assertRaises(subprocess.TimeoutExpired):
                bridge.bounded(command, timeout=0.2)
        self.assertLess(time.monotonic() - started, 2)



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
                self.checker().oracle('verify', 'seed', {}, reject='revision')


class RejectionProtocolTests(unittest.TestCase):
    def test_only_exact_successful_expected_rejection_marker_is_accepted(self):
        checker = SqlBridgeTests().checker()
        marker = b'PASS: expected production codec rejection: reject_revision\n'
        for code, stdout, stderr in [(0, marker, b''), (0, marker, b'error'),
                (3, marker, b''), (0, marker.replace(b'revision', b'identity'), b'')]:
            with self.subTest(code=code, stdout=stdout, stderr=stderr), patch.object(bridge, 'bounded',
                    return_value=subprocess.CompletedProcess([], code, stdout, stderr)) as call:
                if (code, stdout, stderr) == (0, marker, b''):
                    checker.oracle('verify', 'seed', {}, reject='revision')
                    self.assertEqual(call.call_args.args[0][-2:], ['reject_revision', 'seed'])
                else:
                    with self.assertRaises(ValueError):
                        checker.oracle('verify', 'seed', {}, reject='revision')


class NativeScenarioTests(unittest.TestCase):
    def test_required_roundtrips_and_specific_negative_cases_execute(self):
        checker = SqlBridgeTests().checker()
        events = []
        def oracle(mode, label, frame=None, *, reject=None):
            events.append((mode, label, reject))
            return json.dumps({'project': '00', 'configuration': '00', 'revision': '1'}).encode()
        def standalone(fixture, **kwargs):
            label = events[-1][1]
            size = bridge.RECORD_LIMIT + int(label == 'overflow') if label in {'limit', 'overflow'} else 1
            return {'configuration': '00' * size}
        with patch.object(checker, 'oracle', side_effect=oracle), \
                patch.object(checker, 'standalone', side_effect=standalone), \
                patch.object(checker, 'relational', return_value={}) as relational, \
                patch.object(checker, 'sql'), patch('sys.stdout'):
            checker.run()
        self.assertEqual({label for mode, label, reject in events if mode == 'verify' and reject is None},
                         {'seed', 'ordered', 'escaped', 'zero', 'wide', 'maximum', 'limit'})
        for label in ['seed', 'wide', 'maximum']:
            self.assertEqual(events.count(('verify', label, None)), 2)
        self.assertEqual(Counter(reject for _, _, reject in events if reject),
                         {'identity': 2, 'revision': 3, 'archival': 2,
                          'envelope': 2, 'version': 1, 'limit': 1})
        calls = [call.kwargs for call in relational.call_args_list]
        self.assertTrue(any(call.get('revision') == '0' and call.get('message') == 'projects_configuration_revision_check' for call in calls))
        self.assertTrue(any(call.get('next_sequence') == '0' and call.get('message') == 'projects_next_sequence_check' for call in calls))
        self.assertTrue(any(call.get('revision') == '2' for call in calls))
        self.assertTrue(any(call.get('stored_version') == 2 for call in calls))

    def test_invalid_relational_parameters_and_oversized_frame_fail_before_execution(self):
        checker = SqlBridgeTests().checker()
        fixture = {'revision': '1', 'project': '00', 'configuration': '00'}
        with patch.object(checker, 'sql') as sql:
            for kwargs in [{'revision': "1;DELETE"}, {'row_revision': '-1'},
                    {'next_sequence': '1e0'}, {'stored_version': 0}, {'actor': "actor'"}]:
                with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                    checker.relational(fixture, **kwargs)
            sql.assert_not_called()
        with patch.object(bridge, 'FRAME_LIMIT', 32), patch.object(bridge, 'bounded') as run:
            with self.assertRaisesRegex(ValueError, 'input frame overflow'):
                checker.oracle('verify', 'seed', {'large': 'x' * 32})
            run.assert_not_called()

    def test_build_failures_and_missing_artifacts_propagate_and_temporary_target_is_owned(self):
        for outcome in ['build-failure', 'missing-artifact', 'phase-failure', 'success']:
            with self.subTest(outcome=outcome), tempfile.TemporaryDirectory() as root:
                targets = []
                def build(command, **kwargs):
                    target = Path(command[command.index('--target-dir') + 1])
                    self.assertTrue(target.is_relative_to(Path(root) / 'target'))
                    self.assertEqual(command[-2:], ['-j', '1'])
                    targets.append(target)
                    if outcome in {'phase-failure', 'success'}:
                        (target / 'debug').mkdir()
                        (target / 'debug' / ('project_snapshot_check.exe' if os.name == 'nt' else 'project_snapshot_check')).touch()
                    return subprocess.CompletedProcess([], int(outcome == 'build-failure'), b'', b'')
                def phase():
                    self.assertTrue(targets[0].is_dir())
                    if outcome == 'phase-failure': raise ValueError('native phase failed')
                with patch.object(bridge, 'ROOT', Path(root)), patch.object(bridge, 'bounded', side_effect=build), \
                        patch.object(bridge.SnapshotCheck, 'run', side_effect=phase) as check:
                    if outcome == 'success': bridge.run(object())
                    else:
                        with self.assertRaises(ValueError): bridge.run(object())
                    self.assertEqual(check.call_count, int(outcome in {'phase-failure', 'success'}))
                self.assertFalse(targets[0].exists())

    def test_target_resolving_outside_checkout_rejects_before_build_or_temporary_directory(self):
        root = Mock()
        target = root.__truediv__ = Mock(return_value=Mock())
        root.resolve.return_value = Path.cwd()
        target.return_value.resolve.return_value = Path.cwd().parent
        with patch.object(bridge, 'ROOT', root), patch.object(bridge, 'bounded') as build, \
                patch.object(bridge.tempfile, 'TemporaryDirectory') as temporary:
            with self.assertRaisesRegex(ValueError, 'inside the checkout'): bridge.run(object())
            build.assert_not_called()
            temporary.assert_not_called()


class ConstraintOracleTests(unittest.TestCase):
    def test_same_sqlstate_from_a_different_constraint_or_context_does_not_pass(self):
        checker = SqlBridgeTests().checker()
        for stderr in [b'ERROR:  23514: projects_next_sequence_check',
                b'ERROR:  23514: unrelated\nCONTEXT: projects_configuration_revision_check']:
            with self.subTest(stderr=stderr), patch.object(bridge, 'bounded',
                    return_value=subprocess.CompletedProcess([], 3, b'', stderr)), self.assertRaises(ValueError):
                checker.sql('negative', state='23514', message='projects_configuration_revision_check')
        with patch.object(bridge, 'bounded', return_value=subprocess.CompletedProcess([], 3,
                b'', b'ERROR:  23514: violates projects_configuration_revision_check')):
            checker.sql('negative', state='23514', message='projects_configuration_revision_check')

    def test_zero_revision_does_not_also_zero_sequence(self):
        checker = SqlBridgeTests().checker()
        fixture = {'revision': '1', 'project': '00', 'configuration': '00'}
        with patch.object(checker, 'sql') as sql:
            checker.relational(fixture, revision='0')
            self.assertIn("estimate_unit',0,1,'actor'", sql.call_args.args[0])
            checker.relational(fixture, next_sequence='0')
            self.assertIn("estimate_unit',1,0,'actor'", sql.call_args.args[0])


if __name__ == '__main__':
    unittest.main()
