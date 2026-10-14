"""Test-resource ownership must survive startup, test and cleanup failures."""
from pathlib import Path
import json
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import test_services as services


class ServiceOwnershipTests(unittest.TestCase):
    def test_only_requested_services_start_and_owned_project_is_removed(self):
        with patch.object(services, 'run') as run:
            with services.service_group(['docker', 'compose'], ['nats']):
                self.assertEqual(run.call_count, 1)
        start, cleanup = [call.args[0] for call in run.call_args_list]
        self.assertEqual(start[-3:], ['up', '-d', 'nats'])
        self.assertEqual(cleanup[-2:], ['down', '--volumes'])
        self.assertEqual(start[:6], cleanup[:6])
        self.assertRegex(start[3], r'^kehila-test-[0-9a-f]{32}$')

    def test_partial_startup_is_cleaned(self):
        failure = subprocess.CalledProcessError(1, ['compose', 'up'])
        with patch.object(services, 'run', side_effect=[failure, None, None]) as run:
            with self.assertRaises(subprocess.CalledProcessError):
                with services.service_group(['docker', 'compose'], ['postgres', 'ferretdb']):
                    self.fail('startup failure admitted')
        self.assertEqual(run.call_args_list[-1].args[0][-2:], ['down', '--volumes'])

    def test_test_failure_and_cleanup_failure_are_both_visible(self):
        primary, cleanup = RuntimeError('test failed'), OSError('cleanup failed')
        with patch.object(services, 'run', side_effect=[None, None, cleanup]):
            with self.assertRaises(RuntimeError) as caught:
                with services.service_group(['docker', 'compose'], ['nats']):
                    raise primary
        self.assertIs(caught.exception, primary)
        self.assertIs(caught.exception.__cause__, cleanup)

    def test_cleanup_failure_cannot_produce_success(self):
        with patch.object(services, 'run', side_effect=[None, OSError('cleanup failed')]):
            with self.assertRaisesRegex(OSError, 'cleanup failed'):
                with services.service_group(['docker', 'compose'], ['nats']):
                    pass

    def test_invalid_or_empty_service_set_never_starts_resources(self):
        with patch.object(services, 'run') as run:
            for selected in ([], ['unknown'], ['nats', 'nats']):
                with self.subTest(selected=selected), self.assertRaises(ValueError):
                    with services.service_group(['docker', 'compose'], selected):
                        pass
            run.assert_not_called()


class MachineOwnershipTests(unittest.TestCase):
    def test_connection_inventory_must_match_both_machine_endpoints(self):
        info = {'SSHConfig': {'Port': 12345, 'RemoteUsername': 'user'}}
        connections = [{'Name': 'fixture', 'URI': 'ssh://user@127.0.0.1:12345/run/user/1000/podman/podman.sock'},
                       {'Name': 'fixture-root', 'URI': 'ssh://root@127.0.0.1:12345/run/podman/podman.sock'}]
        with patch.object(services, 'run', return_value=Mock(stdout=json.dumps(connections))):
            self.assertEqual(services._machine_connections('fixture', info), ['fixture', 'fixture-root'])
        for invalid in (connections[:1], connections + [connections[0]],
                        [connections[0], {**connections[1], 'URI': 'ssh://root@other-host:12345/socket'}],
                        [connections[0], {**connections[1], 'URI': 'ssh://root@127.0.0.1:54321/socket'}]):
            with self.subTest(invalid=invalid), \
                    patch.object(services, 'run', return_value=Mock(stdout=json.dumps(invalid))):
                with self.assertRaises(ValueError):
                    services._machine_connections('fixture', info)

    def test_stopped_explicit_machine_is_started_and_cleaned(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, 'run') as run, \
                patch.object(services, '_stop_owned_machine') as stop, \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']):
            with services.managed_runtime('podman', 'fixture-machine') as compose:
                self.assertEqual(compose, ['podman', 'compose'])
            run.assert_called_once_with(['podman', 'machine', 'start', 'fixture-machine'], timeout=120)
            stop.assert_called_once_with('fixture-machine')

    def test_already_running_machine_is_never_stopped(self):
        with patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                patch.object(services, 'run') as run, \
                patch.object(services, '_stop_owned_machine') as stop, \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']):
            with services.managed_runtime('podman', 'fixture-machine'):
                pass
            run.assert_not_called()
            stop.assert_not_called()

    def test_failed_machine_start_still_attempts_owned_cleanup(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, 'run', side_effect=OSError('start failed')), \
                patch.object(services, '_stop_owned_machine') as stop:
            with self.assertRaisesRegex(OSError, 'start failed'):
                with services.managed_runtime('podman', 'fixture-machine'):
                    pass
            stop.assert_called_once_with('fixture-machine')

    def test_unrelated_container_in_either_store_prevents_machine_stop(self):
        for outputs in (['container'], ['', 'container']):
            with self.subTest(outputs=outputs), \
                    patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                    patch.object(services, '_machine_connections', return_value=['fixture-machine', 'fixture-machine-root']), \
                    patch.object(services, 'run', side_effect=[Mock(stdout=s) for s in outputs]) as run:
                services._stop_owned_machine('fixture-machine')
                self.assertFalse(any(call.args[0][1:3] == ['machine', 'stop']
                                     for call in run.call_args_list))

    def test_empty_stores_allow_owned_machine_stop(self):
        with patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                patch.object(services, '_machine_connections', return_value=['fixture-machine', 'fixture-machine-root']), \
                patch.object(services, 'run', return_value=Mock(stdout='')) as run:
            services._stop_owned_machine('fixture-machine')
        self.assertEqual(run.call_args_list[-1].args[0],
                         ['podman', 'machine', 'stop', 'fixture-machine'])

    def test_invalid_machine_or_engine_combination_is_rejected_before_io(self):
        with patch.object(services, 'run') as run:
            for engine, machine in (('docker', 'fixture'), ('podman', '../fixture')):
                with self.subTest(engine=engine), self.assertRaises(ValueError):
                    with services.managed_runtime(engine, machine):
                        pass
            run.assert_not_called()
