"""Engine discovery must preserve routing and stop immediately at the work deadline."""
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch
sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'scripts'))
import setup_env
from verification_budget import DeadlineExceeded

class ComposeDiscoveryTests(unittest.TestCase):
    def test_environment_and_bounded_timeout_reach_both_probes(self):
        env={'CONTAINER_CONNECTION':'fixture-root'}
        with patch.object(setup_env.shutil,'which',return_value='found'), \
                patch.object(setup_env,'command_timeout',return_value=2), \
                patch.object(setup_env.subprocess,'run') as run:
            self.assertEqual(setup_env.compose_command('podman',env),['podman','compose'])
        self.assertEqual([call.args[0] for call in run.call_args_list],[['podman','info'],['podman','compose','version']])
        self.assertTrue(all(call.kwargs['env'] is env and call.kwargs['timeout']==2 for call in run.call_args_list))

    def test_auto_falls_back_but_explicit_engine_failure_does_not(self):
        with patch.object(setup_env.shutil,'which',return_value='found'), \
                patch.object(setup_env.subprocess,'run',side_effect=[subprocess.SubprocessError('unavailable'),None,None]) as run:
            self.assertEqual(setup_env.compose_command('auto',{}),['docker','compose'])
        self.assertEqual(run.call_args_list[1].args[0],['docker','info'])
        with patch.object(setup_env.shutil,'which',return_value='found'), \
                patch.object(setup_env.subprocess,'run',side_effect=OSError('unavailable')) as run:
            with self.assertRaises(RuntimeError):setup_env.compose_command('podman',{})
        self.assertEqual(run.call_count,1)

    def test_expired_work_does_not_probe_another_engine(self):
        with patch.object(setup_env.shutil,'which',return_value='found'), \
                patch.object(setup_env,'command_timeout',side_effect=DeadlineExceeded('expired')), \
                patch.object(setup_env.subprocess,'run') as run:
            with self.assertRaises(DeadlineExceeded): setup_env.compose_command('auto',{})
        run.assert_not_called()
