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
            with self.assertRaises(TimeoutError) as caught:
                with sessions.Session(database, 'fixture') as participant:
                    participant.deadline = time.monotonic() + 0.02
                    participant.execute('SELECT fixture_hang;')
            self.assertIsNotNone(participant.process.poll())
            self.assertFalse(getattr(caught.exception, '_cleanup_failures', ()))
        cleanup = [call.args[0] for call in database.sql.call_args_list]
        self.assertTrue(any('pg_terminate_backend' in sql and participant.name in sql for sql in cleanup))
        self.assertFalse(participant.path.exists())

    def test_clean_exit_confirms_backend_absence_and_reaps_process(self):
        database, launched = self.participant()
        with launched:
            with sessions.Session(database, 'fixture') as participant:
                pass
            self.assertIsNotNone(participant.process.poll())
        self.assertFalse(participant.path.exists())
        self.assertIn('SELECT count(*) FROM pg_stat_activity', database.sql.call_args.args[0])

    def test_surviving_backend_after_termination_fails_cleanup(self):
        database, launched = self.participant()
        with launched:
            with self.assertRaisesRegex(ValueError, 'backend remains after cleanup'):
                with sessions.Session(database, 'fixture') as participant:
                    database.sql.return_value = '1\n'
            self.assertIsNotNone(participant.process.poll())
        self.assertFalse(participant.path.exists())

    def test_command_size_boundary_rejection_preserves_idle_state(self):
        database, launched = self.participant()
        with launched, sessions.Session(database, 'fixture') as participant:
            # Size includes UTF-8 SQL bytes, newline, psql marker and its newline.
            suffix = '\n\\echo sp004_done_' + str(participant.serial + 1) + '\n'
            sql_bytes = 4096 - len(suffix.encode('utf-8'))
            sql = 'SELECT ' + 'é' * ((sql_bytes - 7) // 2) + 'x' * ((sql_bytes - 7) % 2)
            serial = participant.serial
            with self.assertRaisesRegex(ValueError, 'command exceeds 4 KiB'):
                participant.send(sql + 'x')
            self.assertEqual(participant.serial, serial)
            self.assertIsNone(participant.pending)
            participant.send(sql)
            self.assertEqual(participant.wait(), '')

    def test_second_command_cannot_overwrite_pending_work(self):
        database, launched = self.participant()
        with launched, sessions.Session(database, 'fixture') as participant:
            participant.send('SELECT 1;')
            pending, serial = participant.pending, participant.serial
            with self.assertRaisesRegex(ValueError, 'already has an in-flight command'):
                participant.send('SELECT 2;')
            self.assertEqual((participant.pending, participant.serial), (pending, serial))
            self.assertEqual(participant.wait(), '')

    def test_poll_requires_pending_work_and_bounds_cumulative_output(self):
        database, launched = self.participant()
        with launched, sessions.Session(database, 'fixture') as participant:
            with self.assertRaisesRegex(ValueError, 'has no in-flight command'):
                participant.poll()
            participant.send('SELECT 1;')
            self.assertEqual(participant.wait(), '')
            # Completed output counts toward the lifetime limit, even when the
            # unread result is a small, valid marker frame.
            with participant.path.open('r+b') as output:
                output.truncate(65536)
            participant.offset = 65536
            participant.send('SELECT fixture_hang;')
            with participant.path.open('ab') as output:
                output.write((participant.pending + '\n').encode('utf-8'))
            with self.assertRaisesRegex(ValueError, 'output exceeds 64 KiB'):
                participant.poll()

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
            with self.assertRaisesRegex(TimeoutError, 'exceeded 60 seconds') as caught:
                with sessions.Session(database, 'fixture') as participant:
                    delay, expire = timer.call_args.args
                    self.assertEqual(delay, 60)
                    timer.return_value.start.assert_called_once_with()
                    expire()  # Trigger the supervised deadline while the caller is idle.
                    participant.process.wait(timeout=5)
                    self.assertIsNotNone(participant.process.poll())
            timer.return_value.cancel.assert_called_once()
            self.assertFalse(getattr(caught.exception, '_cleanup_failures', ()))

    def test_expiry_during_watchdog_stop_remains_primary_with_cleanup_failures(self):
        for supervisor_fails, backend_fails in ((False, False), (True, False), (False, True), (True, True)):
            database, launched = self.participant()
            supervisor_error = OSError('watchdog cleanup failed')
            backend_error = OSError('backend cleanup failed')
            with self.subTest(supervisor=supervisor_fails, backend=backend_fails), launched, \
                    patch.object(sessions.threading, 'Timer') as timer:
                timer.return_value.ident = None
                timer.return_value.is_alive.return_value = False
                with self.assertRaisesRegex(TimeoutError, 'exceeded 60 seconds') as caught:
                    with sessions.Session(database, 'fixture') as participant:
                        # Deterministically expire between entering __exit__ and
                        # joining the supervisor, rather than depending on timing.
                        timer.return_value.cancel.side_effect = participant.expire
                        if supervisor_fails:
                            participant.watchdog_error = supervisor_error
                        if backend_fails:
                            database.sql.side_effect = backend_error
                failures = [error for _, error in getattr(caught.exception, '_cleanup_failures', ())]
                self.assertEqual(failures, [error for condition, error in
                    ((supervisor_fails, supervisor_error), (backend_fails, backend_error)) if condition])
                timer.return_value.cancel.assert_called_once()
                self.assertIsNotNone(participant.process.poll())
                self.assertFalse(participant.path.exists())

    def test_body_error_remains_primary_when_watchdog_cleanup_fails(self):
        database, launched = self.participant()
        body_error = ValueError('body failed')
        watchdog_error = OSError('watchdog cleanup failed')
        with launched, patch.object(sessions.threading, 'Timer') as timer:
            timer.return_value.ident = None
            timer.return_value.is_alive.return_value = False
            with self.assertRaisesRegex(ValueError, 'body failed') as caught:
                with sessions.Session(database, 'fixture') as participant:
                    participant.watchdog_error = watchdog_error
                    raise body_error
            self.assertIs(caught.exception, body_error)
            self.assertEqual([error for _, error in body_error._cleanup_failures], [watchdog_error])
            self.assertIsNotNone(participant.process.poll())

    def test_watchdog_start_failure_still_reaps_participant(self):
        database, launched = self.participant()
        participant = sessions.Session(database, 'fixture')
        with launched, patch.object(sessions.threading, 'Timer') as timer:
            timer.return_value.ident = None
            timer.return_value.is_alive.return_value = False
            timer.return_value.start.side_effect = OSError('watchdog startup failed')
            with self.assertRaisesRegex(OSError, 'watchdog startup failed') as caught:
                with participant:
                    self.fail('startup must fail before entering the body')
            self.assertFalse(getattr(caught.exception, '_cleanup_failures', ()))
            timer.return_value.cancel.assert_called_once()
            self.assertIsNotNone(participant.process.poll())
            self.assertFalse(participant.path.exists())

    def test_process_start_failure_cleans_files_without_a_watchdog(self):
        database, _ = self.participant()
        participant = sessions.Session(database, 'fixture')
        with patch.object(sessions.subprocess, 'Popen', side_effect=OSError('process startup failed')):
            with self.assertRaisesRegex(OSError, 'process startup failed') as caught:
                with participant:
                    self.fail('startup must fail before entering the body')
        self.assertFalse(getattr(caught.exception, '_cleanup_failures', ()))
        self.assertFalse(participant.path.exists())
        database.sql.assert_not_called()


if __name__ == '__main__':
    unittest.main()
