"""Test-resource ownership must survive startup, test and cleanup failures."""
from pathlib import Path
from contextlib import nullcontext
import json
import tempfile
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import test_services as services


class ServiceOwnershipTests(unittest.TestCase):
    def test_only_requested_services_start_and_owned_project_is_removed(self):
        with patch.object(services, 'run') as run, \
                patch.object(services, '_containers', side_effect=[['container'], []]):
            with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['nats']):
                self.assertEqual(run.call_count, 1)
        start, cleanup = [call.args[0] for call in run.call_args_list]
        self.assertEqual(start[-3:], ['up', '-d', 'nats'])
        self.assertEqual(cleanup[-2:], ['down', '--volumes'])
        self.assertEqual(start[:6], cleanup[:6])
        self.assertRegex(start[3], r'^kehila-test-[0-9a-f]{32}$')

    def test_partial_startup_is_cleaned(self):
        failure = subprocess.CalledProcessError(1, ['compose', 'up'])
        with patch.object(services, 'run', side_effect=[failure, None, None]) as run, \
                patch.object(services, '_containers', return_value=[]):
            with self.assertRaises(subprocess.CalledProcessError):
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['postgres', 'ferretdb']):
                    self.fail('startup failure admitted')
        self.assertEqual(run.call_args_list[-1].args[0][-2:], ['down', '--volumes'])

    def test_test_failure_and_cleanup_failure_are_both_visible(self):
        primary, cleanup = RuntimeError('test failed'), OSError('cleanup failed')
        with patch.object(services, 'run', side_effect=[None, None, cleanup]), \
                patch.object(services, '_containers', return_value=['container']):
            with self.assertRaises(RuntimeError) as caught:
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['nats']):
                    raise primary
        self.assertIs(caught.exception, primary)
        self.assertIs(caught.exception.__cause__, cleanup)

    def test_cleanup_failure_cannot_produce_success(self):
        with patch.object(services, 'run', side_effect=[None, OSError('cleanup failed')]), \
                patch.object(services, '_containers', return_value=['container']):
            with self.assertRaisesRegex(OSError, 'cleanup failed'):
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['nats']):
                    pass

    def test_invalid_or_empty_service_set_never_starts_resources(self):
        with patch.object(services, 'run') as run:
            for selected in ([], ['unknown'], ['nats', 'nats']):
                with self.subTest(selected=selected), self.assertRaises(ValueError):
                    with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), selected):
                        pass
            run.assert_not_called()

    def test_successful_start_without_owned_running_container_is_rejected(self):
        with patch.object(services, 'run'), patch.object(services, '_containers', return_value=[]):
            with self.assertRaisesRegex(RuntimeError, 'one running owned container'):
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['nats']):
                    self.fail('unowned service admitted')

    def test_successful_down_with_remaining_containers_still_fails(self):
        with patch.object(services, 'run'), \
                patch.object(services, '_containers', side_effect=[['container'], ['leftover']]):
            with self.assertRaisesRegex(RuntimeError, 'remain after Compose cleanup'):
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['nats']):
                    pass

    def test_container_queries_are_scoped_to_project_and_service(self):
        with patch.object(services, 'run', return_value=Mock(stdout='abc123\n')) as run:
            self.assertEqual(services._containers(services.ComposeRuntime(['podman', 'compose'], {}), 'owned', 'nats'), ['abc123'])
            self.assertEqual(run.call_args.args[0], ['podman', 'ps', '--filter',
                'label=com.docker.compose.project=owned', '--filter',
                'label=com.docker.compose.service=nats', '--format', '{{.ID}}'])
            services._containers(services.ComposeRuntime(['podman', 'compose'], {}), 'owned', include_stopped=True)
            self.assertIn('--all', run.call_args.args[0])


class MachineOwnershipTests(unittest.TestCase):
    def setUp(self):
        lease = patch.object(services, '_machine_lease', return_value=nullcontext())
        lease.start()
        self.addCleanup(lease.stop)

    def test_shared_stopped_machine_is_started_but_never_automatically_stopped(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, '_machine_connections', return_value=['fixture', 'fixture-root']), \
                patch.object(services, 'run'), patch.object(services, '_stop_owned_machine') as stop, \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']):
            with services.managed_runtime('podman', 'fixture', environment={}):
                pass
            stop.assert_not_called()

    def test_exclusive_mode_refuses_an_already_running_machine(self):
        with patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                patch.object(services, '_machine_connections', return_value=['fixture', 'fixture-root']), \
                patch.object(services, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'stopped'):
                with services.managed_runtime('podman', 'fixture', environment={}, exclusive=True):
                    pass
            run.assert_not_called()

    def test_stopped_machine_cleanup_does_not_query_connections(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, '_machine_connections') as connections, \
                patch.object(services, 'run') as run:
            services._stop_owned_machine('fixture')
            connections.assert_not_called()
            run.assert_not_called()

    def test_exclusive_cleanup_can_stop_its_partially_started_machine(self):
        with patch.object(services, '_machine_info', return_value={'State': 'starting'}), \
                patch.object(services, 'run') as run:
            services._stop_owned_machine('fixture')
            self.assertEqual(run.call_args.args[0], ['podman', 'machine', 'stop', 'fixture'])

    def test_runtime_cleanup_failure_preserves_the_primary_error(self):
        primary = RuntimeError('test failed')
        cleanup = OSError('VM stop failed')
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, '_machine_connections', return_value=['fixture', 'fixture-root']), \
                patch.object(services, 'run'), \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']), \
                patch.object(services, '_stop_owned_machine', side_effect=cleanup):
            with self.assertRaises(RuntimeError) as caught:
                with services.managed_runtime('podman', 'fixture', environment={}, exclusive=True):
                    raise primary
            self.assertIs(caught.exception, primary)
            self.assertIs(caught.exception.__cause__, cleanup)

    def test_socket_paths_must_identify_distinct_rootless_and_rootful_stores(self):
        info = {'SSHConfig': {'Port': 12345, 'RemoteUsername': 'user'}}
        rootless = {'Name': 'fixture', 'URI': 'ssh://user@127.0.0.1:12345/run/user/1000/podman/podman.sock'}
        for path in ('', '/run/user/1000/podman/podman.sock', '/arbitrary'):
            connections = [rootless, {'Name': 'fixture-root', 'URI': 'ssh://root@127.0.0.1:12345' + path}]
            with self.subTest(path=path), patch.object(services, 'run', return_value=Mock(stdout=json.dumps(connections))):
                with self.assertRaises(ValueError):
                    services._machine_connections('fixture', info)

    def test_missing_connections_prevent_machine_start(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, '_machine_connections', side_effect=ValueError('missing connection')), \
                patch.object(services, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'missing connection'):
                with services.managed_runtime('podman', 'fixture', environment={}):
                    self.fail('missing preflight admitted')
            run.assert_not_called()

    def test_foreign_connection_is_rejected_before_machine_start(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, '_machine_connections', return_value=['fixture', 'fixture-root']), \
                patch.object(services, 'run') as run:
            with self.assertRaises(ValueError):
                with services.managed_runtime('podman', 'fixture', environment={'CONTAINER_CONNECTION': 'other'}):
                    pass
            run.assert_not_called()

    def test_requested_connection_is_carried_to_compose_and_engine(self):
        with patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                patch.object(services, '_machine_connections', return_value=['fixture', 'fixture-root']), \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']) as compose:
            with services.managed_runtime('podman', 'fixture', environment={'CONTAINER_CONNECTION': 'fixture-root'}) as runtime:
                self.assertEqual(runtime.engine, ['podman', '--connection', 'fixture-root'])
                self.assertEqual(runtime.environment['CONTAINER_CONNECTION'], 'fixture-root')
                compose.assert_called_once_with('podman', environment=runtime.environment)

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
                patch.object(services, '_machine_connections', return_value=['fixture-machine', 'fixture-machine-root']), \
                patch.object(services, 'run') as run, \
                patch.object(services, '_stop_owned_machine') as stop, \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']):
            with services.managed_runtime('podman', 'fixture-machine', environment={}, exclusive=True) as compose:
                self.assertEqual(compose.compose, ['podman', 'compose'])
            run.assert_called_once_with(['podman', 'machine', 'start', 'fixture-machine'], timeout=120, env={'CONTAINER_CONNECTION': 'fixture-machine'})
            stop.assert_called_once_with('fixture-machine', environment={'CONTAINER_CONNECTION': 'fixture-machine'})

    def test_already_running_machine_is_never_stopped(self):
        with patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                patch.object(services, '_machine_connections', return_value=['fixture-machine', 'fixture-machine-root']), \
                patch.object(services, 'run') as run, \
                patch.object(services, '_stop_owned_machine') as stop, \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']):
            with services.managed_runtime('podman', 'fixture-machine', environment={}):
                pass
            run.assert_not_called()
            stop.assert_not_called()

    def test_failed_machine_start_still_attempts_owned_cleanup(self):
        with patch.object(services, '_machine_info', return_value={'State': 'stopped'}), \
                patch.object(services, '_machine_connections', return_value=['fixture-machine', 'fixture-machine-root']), \
                patch.object(services, 'run', side_effect=OSError('start failed')), \
                patch.object(services, '_stop_owned_machine') as stop:
            with self.assertRaisesRegex(OSError, 'start failed'):
                with services.managed_runtime('podman', 'fixture-machine', environment={}, exclusive=True):
                    pass
            stop.assert_called_once_with('fixture-machine', environment={'CONTAINER_CONNECTION': 'fixture-machine'})

    def test_unrelated_container_in_either_store_prevents_machine_stop(self):
        for outputs in (['container'], ['', 'container']):
            with self.subTest(outputs=outputs), \
                    patch.object(services, '_machine_info', return_value={'State': 'running'}), \
                    patch.object(services, '_machine_connections', return_value=['fixture-machine', 'fixture-machine-root']), \
                    patch.object(services, 'run', side_effect=[Mock(stdout=s) for s in outputs]) as run:
                with self.assertRaisesRegex(RuntimeError, 'exclusive'):
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


class MachineLeaseTests(unittest.TestCase):
    def test_lease_excludes_a_second_verifier_and_releases_after_failure(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(services.tempfile, 'gettempdir', return_value=folder):
            with self.assertRaisesRegex(RuntimeError, 'body failed'):
                with services._machine_lease('fixture'):
                    with self.assertRaisesRegex(RuntimeError, 'Another verifier'):
                        with services._machine_lease('fixture'):
                            self.fail('overlapping verifier admitted')
                    raise RuntimeError('body failed')
            with services._machine_lease('fixture'):
                pass
