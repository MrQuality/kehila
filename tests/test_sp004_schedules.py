"""A missed prerequisite must never qualify a native negative oracle."""
from pathlib import Path
import sys
import unittest
from unittest.mock import MagicMock, Mock, patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'experiments/SP-004'))
import concurrency as schedules
from sessions import SqlFailure


class ScheduleOracleTests(unittest.TestCase):
    def test_timeout_during_startup_is_not_evidence_of_transaction_cleanup(self):
        with patch.object(schedules, 'Session') as session, \
                patch.object(schedules, 'check_project'), \
                patch.object(schedules, 'query', side_effect=[True, 0]):
            session.return_value.__enter__.side_effect = TimeoutError('native participant exceeded 60 seconds')
            with self.assertRaisesRegex(ValueError, 'timeout probe did not reach'):
                schedules.timeout_cleanup(Mock())

    def test_timeout_requires_a_reached_uncommitted_grant_change(self):
        with patch.object(schedules, 'Session') as session, \
                patch.object(schedules, 'check_project'), \
                patch.object(schedules, 'query', side_effect=[True, 0]):
            participant = session.return_value.__enter__.return_value
            participant.execute.side_effect = ['1', TimeoutError('native participant exceeded 60 seconds')]
            with self.assertRaisesRegex(ValueError, 'uncommitted grant change was not reached'):
                schedules.timeout_cleanup(Mock())

    def test_timeout_requires_a_reached_uncommitted_operation_and_payload(self):
        with patch.object(schedules, 'Session') as session, \
                patch.object(schedules, 'check_project') as checked, \
                patch.object(schedules, 'query', side_effect=[True, 0]) as queried:
            participant = session.return_value.__enter__.return_value
            participant.execute.side_effect = ['2', 'f', TimeoutError('native participant exceeded 60 seconds')]
            with self.assertRaisesRegex(ValueError, 'uncommitted management operation was not reached'):
                schedules.timeout_cleanup(Mock())
            checked.assert_not_called()
            queried.assert_not_called()

    def test_timeout_with_cleanup_failure_cannot_qualify_rollback(self):
        error = TimeoutError('native participant exceeded 60 seconds')
        error._cleanup_failures = (('native participant', OSError('backend remains')),)
        with patch.object(schedules, 'Session') as session, \
                patch.object(schedules, 'check_project') as checked, \
                patch.object(schedules, 'query', side_effect=[True, 0]) as queried:
            participant = session.return_value.__enter__.return_value
            participant.execute.side_effect = ['2', 't', error]
            with self.assertRaises(TimeoutError) as caught:
                schedules.timeout_cleanup(Mock())
            self.assertIs(caught.exception, error)
            self.assertEqual(participant.execute.call_count, 3)
            checked.assert_not_called()
            queried.assert_not_called()

    def test_clean_timeout_checks_rollback_before_qualifying(self):
        database = Mock()
        with patch.object(schedules, 'Session') as session, \
                patch.object(schedules, 'check_project') as checked, \
                patch.object(schedules, 'query', side_effect=[True, 0]) as queried:
            participant = session.return_value.__enter__.return_value
            participant.execute.side_effect = ['2', 't', TimeoutError('native participant exceeded 60 seconds')]
            self.assertIn('grant/event/operation rolled back', schedules.timeout_cleanup(database))
            checked.assert_called_once_with(database, 'timeout-cleanup', True)
            self.assertEqual(queried.call_count, 2)

    def test_unexpected_authority_success_is_a_typed_outcome(self):
        participant = Mock()
        participant.wait.return_value = 'created'
        with self.assertRaises(schedules.UnexpectedAuthorityOutcome) as caught:
            schedules.denial(participant, 'creation')
        self.assertEqual(caught.exception.outcome, 'created')
        participant.wait.side_effect = SqlFailure('40001', 'other failure')
        with self.assertRaises(SqlFailure):
            schedules.denial(participant, 'creation')

    def test_lock_order_failure_requires_the_later_actor_barrier_first(self):
        for later_reached in (False, True):
            participants = [MagicMock() for _ in range(4)]
            for participant in participants:
                participant.__enter__.return_value = participant
                participant.__exit__.return_value = False
            missed = schedules.BarrierNotReached(1, 2)
            with self.subTest(later_reached=later_reached), \
                    patch.object(schedules, 'Session', side_effect=participants), \
                    patch.object(schedules, 'blocked', side_effect=([{'holder': 3, 'waiter': 4}, missed]
                                 if later_reached else [missed])):
                expected = schedules.LockOrderViolation if later_reached else schedules.BarrierNotReached
                with self.assertRaises(expected) as caught:
                    schedules.lock_order(Mock(), 'admin_actor', 'actor', 'fixture', 'probe')
                if later_reached:
                    self.assertEqual(caught.exception.later, {'holder': 3, 'waiter': 4})


if __name__ == '__main__':
    unittest.main()
