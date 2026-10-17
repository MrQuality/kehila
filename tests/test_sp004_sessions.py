"""Bounded result framing for native concurrency participants."""
import importlib.util
from pathlib import Path
import unittest
import subprocess
import sys
import time
from unittest.mock import Mock, patch

spec = importlib.util.spec_from_file_location(
    'sp004_sessions', Path(__file__).resolve().parents[1] / 'experiments/SP-004/sessions.py')
sessions = importlib.util.module_from_spec(spec)
spec.loader.exec_module(sessions)


class FrameTests(unittest.TestCase):
    def test_only_a_complete_line_finishes_a_command(self):
        self.assertIsNone(sessions.frame(b'13\nmarker', 'marker'))
        self.assertEqual(sessions.frame(b'13\nmarker\n', 'marker'), '13')
        self.assertEqual(sessions.frame(b'13\r\nmarker\r\n', 'marker'), '13')
        self.assertIsNone(sessions.frame(b'not-marker\n', 'marker'))

    def test_sql_error_preserves_specific_state_and_primary_diagnostic(self):
        with self.assertRaises(sessions.SqlFailure) as caught:
            sessions.frame(b'ERROR:  42501: fixture authority denied\nCONTEXT: ignored\n', 'marker')
        self.assertEqual(caught.exception.state, '42501')
        self.assertEqual(caught.exception.primary, 'fixture authority denied')

    def test_invalid_utf8_and_overflow_fail_instead_of_truncating(self):
        for data in (b'\xff', b'x' * 65537):
            with self.subTest(length=len(data)), self.assertRaises(ValueError):
                sessions.frame(data, 'marker')


class OwnershipTests(unittest.TestCase):
    def participant(self):
        database = Mock(podman=['podman'], container='fixture')
        database.sql.return_value = '0\n'
        real_popen = subprocess.Popen
        program = ("import sys,time\nfor line in sys.stdin:\n"
                   " if 'pg_backend_pid' in line: print('123',flush=True)\n"
                   " if 'fixture_hang' in line: time.sleep(30)\n"
                   " if line.startswith('\\\\echo '): print(line.split()[1],flush=True)\n")
        # Psql is replaced only for process-lifetime tests, never SQL evidence.
        def launch(command, **options):
            return real_popen([sys.executable, '-u', '-c', program], **options)
        return database, patch.object(sessions.subprocess, 'Popen', side_effect=launch)

    def test_command_timeout_reaps_process_and_targets_only_owned_backend(self):
        database, launched = self.participant()
        with launched:
            with self.assertRaises(TimeoutError):
                with sessions.Session(database, 'fixture') as participant:
                    participant.deadline = time.monotonic() + 0.02
                    participant.execute('SELECT fixture_hang;')
            self.assertIsNotNone(participant.process.poll())
        cleanup = [call.args[0] for call in database.sql.call_args_list]
        self.assertTrue(any('pg_terminate_backend' in sql and participant.name in sql for sql in cleanup))
        self.assertFalse(participant.path.exists())

    def test_backend_cleanup_failure_preserves_body_error_and_reaps_local_process(self):
        database, launched = self.participant()
        failure = ValueError('body failed')
        with launched:
            with self.assertRaisesRegex(ValueError, 'body failed') as caught:
                with sessions.Session(database, 'fixture') as participant:
                    database.sql.side_effect = OSError('backend cleanup failed')
                    raise failure
            self.assertIs(caught.exception, failure)
            self.assertIn('backend cleanup failed', str(failure._cleanup_failures[0][1]))
            self.assertIsNotNone(participant.process.poll())

    def test_completed_waiter_cannot_be_credited_as_a_reached_barrier(self):
        observer = Mock()
        observer.execute.return_value = 'f'
        waiter, holder = Mock(pid=2, deadline=time.monotonic() + 60), Mock(pid=1, deadline=time.monotonic() + 60)
        waiter.poll.return_value = 'completed'
        with self.assertRaisesRegex(ValueError, 'wait was not reached'):
            sessions.blocked(observer, waiter, holder)

    def test_deadline_kills_idle_participant_without_waiting_for_another_command(self):
        database, launched = self.participant()
        with launched, patch.object(sessions.threading, 'Timer') as timer:
            timer.return_value.ident = None
            timer.return_value.is_alive.return_value = False
            with self.assertRaisesRegex(TimeoutError, 'exceeded 60 seconds'):
                with sessions.Session(database, 'fixture') as participant:
                    delay, expire = timer.call_args.args
                    self.assertEqual(delay, 60)
                    expire()  # Trigger the supervised deadline while the caller is idle.
                    participant.process.wait(timeout=5)
                    self.assertIsNotNone(participant.process.poll())
            timer.return_value.cancel.assert_called_once()


if __name__ == '__main__':
    unittest.main()
