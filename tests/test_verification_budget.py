"""The verifier owns its deadline and keeps bounded teardown time available."""
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import verification_budget as budget
import test_services as services


class DeadlineTests(unittest.TestCase):
    def test_work_timeout_is_clamped_and_expired_work_is_not_started(self):
        now = [100]
        with patch.object(budget.time, 'monotonic', side_effect=lambda: now[0]):
            with budget.work_budget(10):
                self.assertEqual(budget.command_timeout(30), 10)
                self.assertEqual(budget.command_timeout(2), 2)
                now[0] = 110
                with self.assertRaises(budget.DeadlineExceeded):
                    budget.command_timeout(30)
                with budget.cleanup_time():
                    self.assertEqual(budget.command_timeout(30), 30)
                with self.assertRaises(budget.DeadlineExceeded):
                    budget.command_timeout(30)
        self.assertEqual(budget.command_timeout(30), 30)

    def test_invalid_budgets_are_rejected(self):
        for seconds in (0, -1, float('nan'), float('inf'), 601):
            with self.subTest(seconds=seconds), self.assertRaises(ValueError):
                with budget.work_budget(seconds):
                    self.fail('invalid work budget admitted')

    def test_expired_service_work_still_logs_and_tears_down_with_separate_limits(self):
        now = [0]
        runtime = services.ComposeRuntime(['docker', 'compose'], {})
        with patch.object(budget.time, 'monotonic', side_effect=lambda: now[0]), \
                patch.object(services.subprocess, 'run') as run, \
                patch.object(services, '_containers', side_effect=[['container'], []]):
            with self.assertRaises(budget.DeadlineExceeded):
                with budget.work_budget(10):
                    with services.service_group(runtime, ['nats']):
                        now[0] = 20
                        services.run(['work-that-must-not-start'])
            self.assertEqual([call.kwargs['timeout'] for call in run.call_args_list], [10, 30, 120])
            self.assertEqual(run.call_args_list[-1].args[0][-2:], ['down', '--volumes'])

    def test_child_timeout_returns_control_to_the_owner(self):
        with self.assertRaises(subprocess.TimeoutExpired):
            with budget.work_budget(0.2):
                subprocess.run([sys.executable, '-c', 'import time; time.sleep(5)'],
                               check=True, timeout=budget.command_timeout(10))
        self.assertEqual(budget.command_timeout(10), 10)
