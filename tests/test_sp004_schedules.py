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
