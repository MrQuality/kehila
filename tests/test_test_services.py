"""Test-resource ownership must survive startup, test and cleanup failures."""
from pathlib import Path
from contextlib import nullcontext
import hashlib
import json
import os
import tempfile
import io
import subprocess
import sys
import unittest
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import test_services as services
from verification_ownership import ServiceOwnership, EMPTY


class ServiceOwnershipTests(unittest.TestCase):
    def test_primary_failure_survives_tracked_cleanup_ambiguity(self):
        primary = RuntimeError('original workload failure')
        cleanup = ValueError('ambiguous resource')
        ownership = Mock()
        group = ownership.group.return_value
        group.project = 'kehila-test-' + 'a' * 32
        group.cleanup.side_effect = cleanup
        with patch.object(services, 'run'), \
                patch.object(services, '_containers', return_value=['owned-search']), \
                patch('verification_service_resources.ComposeResources'):
            with self.assertRaises(RuntimeError) as caught:
                with services.service_group(services.ComposeRuntime(['podman', 'compose'], {}),
                                            ['opensearch'], ownership=ownership):
                    raise primary
        self.assertIs(caught.exception, primary)
        self.assertEqual(primary._cleanup_failures, (('Compose project', cleanup),))

    def test_partial_start_keeps_volume_binding_before_destructive_cleanup(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source'
            source.mkdir()
            fixture = source / 'docker-compose.yml'
            fixture.write_text('services: {}', encoding='utf-8')
            runtime = services.ComposeRuntime(['podman', 'compose'], {})
            record = root / 'record.json'
            receipt = ServiceOwnership.create(record, runtime, source, 'a' * 32, [('opensearch',)])
            resources = Mock()
            resources.inventory.return_value = EMPTY
            cid, volume = 'b' * 64, 'e' * 64
            project = receipt.group(['opensearch'], fixture).project
            live = {'containers': {cid: {'service': 'opensearch', 'name': project + '_opensearch_1'}}, 'networks': {},
                    'volumes': {volume: {'container': cid, 'destination': '/data', 'created_at': 'synthetic'}}}
            primary = RuntimeError('partial startup')

            def command(arguments, **kwargs):
                if 'up' in arguments:
                    resources.inventory.return_value = live
                    raise primary

            def remove(*args):
                saved = json.loads(record.read_text())['groups'][0]
                self.assertEqual(saved['state'], 'observed')
                self.assertIn(volume, saved['resources']['volumes'])
                # Model interruption after its container is gone: namespace
                # discovery alone can no longer recover this volume binding.
                resources.inventory.return_value = EMPTY
                raise RuntimeError('retained anonymous volume')

            resources.remove.side_effect = remove
            with patch.object(services, 'run', side_effect=command), \
                    patch.object(services, '_containers', return_value=[]), \
                    patch('verification_service_resources.ComposeResources', return_value=resources):
                with self.assertRaises(RuntimeError) as caught:
                    with services.service_group(runtime, ['opensearch'], compose_file=fixture, ownership=receipt):
                        self.fail('failed startup entered body')
            self.assertIs(caught.exception, primary)
            saved = json.loads(record.read_text())['groups'][0]
            self.assertEqual(saved['state'], 'observed')
            self.assertIn(volume, saved['resources']['volumes'])
            self.assertEqual(str(primary._cleanup_failures[0][1]), 'retained anonymous volume')

    def test_unarmed_occupied_group_cannot_enter_destructive_finally(self):
        ownership = Mock()
        ownership.group.return_value.arm.side_effect = ValueError('occupied namespace')
        with patch.object(services, 'run') as command:
            with self.assertRaisesRegex(ValueError, 'occupied namespace'):
                with services.service_group(services.ComposeRuntime(['podman', 'compose'], {}),
                                            ['opensearch'], ownership=ownership):
                    self.fail('occupied group entered')
        command.assert_not_called()
        ownership.group.return_value.cleanup.assert_not_called()

    def test_tracked_group_observes_before_body_and_uses_verified_cleanup(self):
        events = []
        ownership = Mock()
        group = ownership.group.return_value
        group.project = 'kehila-test-' + 'a' * 32
        group.arm.side_effect = lambda resources: events.append('armed')
        group.observe.side_effect = lambda resources, **kwargs: events.append('observed')
        group.cleanup.side_effect = lambda resources: events.append('verified-cleanup')
        with patch.object(services, 'run', side_effect=lambda *args, **kwargs: events.append(args[0][-1])), \
                patch.object(services, '_containers', side_effect=[['owned-search'], []]), \
                patch('verification_service_resources.ComposeResources'):
            with services.service_group(services.ComposeRuntime(['podman', 'compose'], {}),
                                        ['opensearch'], ownership=ownership):
                events.append('body')
        self.assertEqual(events, ['armed', 'opensearch', 'observed', 'body', 'verified-cleanup'])
        self.assertEqual(group.observe.call_args.kwargs['expected_containers'], {'opensearch': 'owned-search'})

    def test_machine_inspection_uses_stable_account_directory(self):
        stable = Path(tempfile.gettempdir()).resolve()
        with patch.object(services, '_account_directory', return_value=stable), \
                patch.object(services.subprocess, 'run', return_value=Mock(
                    stdout='[{"Name":"fixture","State":"stopped"}]')) as run:
            services._machine_info('fixture', environment={})
        self.assertEqual(run.call_args.kwargs['cwd'], stable)

    def test_service_commands_keep_the_source_working_directory(self):
        with patch.object(services.subprocess, 'run') as run:
            services.run(['podman', 'compose', '-f', str(services.ROOT / 'docker-compose.yml'), 'config'])
        self.assertEqual(run.call_args.kwargs['cwd'], services.ROOT)

    def test_group_yields_the_verified_container_identity_for_each_service(self):
        with patch.object(services, 'run'), \
                patch.object(services, '_containers',
                             side_effect=[['owned-postgres'], ['owned-search'], []]) as inventory:
            with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}),
                                        ['postgres', 'opensearch']) as containers:
                self.assertEqual(containers, {'postgres': 'owned-postgres',
                                              'opensearch': 'owned-search'})
        inspected = inventory.call_args_list
        self.assertEqual(inspected[0].args[1], inspected[1].args[1])
        self.assertEqual(inspected[0].args[1], inspected[2].args[1])

    def test_custom_fixture_is_used_for_start_and_cleanup(self):
        with tempfile.TemporaryDirectory() as folder:
            fixture = Path(folder) / 'compose.yml'
            fixture.write_text('services: {}', encoding='utf-8')
            with patch.object(services, 'run') as run, \
                    patch.object(services, '_containers', side_effect=[['container'], []]):
                with services.service_group(services.ComposeRuntime(['docker','compose'], {}),
                                            ['postgres'], compose_file=fixture):
                    pass
            start, cleanup = [call.args[0] for call in run.call_args_list]
            self.assertEqual(start[start.index('-f') + 1], str(fixture.resolve()))
            self.assertEqual(cleanup[cleanup.index('-f') + 1], str(fixture.resolve()))
            self.assertEqual(start[start.index('-p') + 1], cleanup[cleanup.index('-p') + 1])

    def test_failed_diagnostics_do_not_replace_the_test_failure(self):
        primary = RuntimeError('test failed')
        with patch.object(services, 'run', side_effect=[None, OSError('logs failed'), None]), \
                patch.object(services, '_containers', side_effect=[['container'], []]), \
                patch('sys.stderr', new_callable=io.StringIO) as stderr:
            with self.assertRaises(RuntimeError) as caught:
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['nats']):
                    raise primary
            self.assertIs(caught.exception, primary)
            self.assertIn('logs failed', stderr.getvalue())

    def test_failure_logs_have_a_line_and_time_bound(self):
        with patch.object(services,'run') as run, \
                patch.object(services,'_containers',side_effect=[['container'],[]]):
            with self.assertRaisesRegex(RuntimeError,'failed'):
                with services.service_group(services.ComposeRuntime(['docker','compose'],{}),['nats']):
                    raise RuntimeError('failed')
        logs=[call for call in run.call_args_list if 'logs' in call.args[0]]
        self.assertEqual(len(logs),1)
        self.assertEqual(logs[0].args[0][-4:],['logs','--no-color','--tail','200'])
        self.assertEqual(logs[0].kwargs['timeout'],30)

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
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['postgres', 'nats']):
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
        self.assertIs(caught.exception._cleanup_failures[0][1], cleanup)
        self.assertIsNone(caught.exception.__cause__)

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

    def test_multiple_matching_containers_are_rejected_and_owned_group_is_cleaned(self):
        with patch.object(services, 'run') as run, \
                patch.object(services, '_containers', side_effect=[['first', 'second'], []]):
            with self.assertRaisesRegex(RuntimeError, 'one running owned container'):
                with services.service_group(services.ComposeRuntime(['docker', 'compose'], {}), ['postgres']):
                    self.fail('ambiguous service admitted')
        self.assertEqual(run.call_args_list[-1].args[0][-2:], ['down', '--volumes'])

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
                'label=com.docker.compose.service=nats', '--no-trunc', '--format', '{{.ID}}'])
            services._containers(services.ComposeRuntime(['podman', 'compose'], {}), 'owned', include_stopped=True)
            self.assertIn('--all', run.call_args.args[0])


class MachineOwnershipTests(unittest.TestCase):
    def setUp(self):
        lease = patch.object(services, '_machine_lease', return_value=nullcontext())
        lease.start()
        self.addCleanup(lease.stop)

    def test_machine_info_rejects_malformed_wrong_multiple_and_unstable_records(self):
        invalid = ['not json', 'null', '[]', '[null]', '[{},{}]',
                   json.dumps([{'Name': 'other', 'State': 'running'}]),
                   json.dumps([{'Name': 'fixture', 'State': 'starting'}]),
                   json.dumps([{'Name': 'fixture', 'State': 'invalid'}])]
        for raw in invalid:
            with self.subTest(raw=raw), patch.object(services, 'run', return_value=Mock(stdout=raw)):
                with self.assertRaises(ValueError):
                    services._machine_info('fixture')
        for state in ('stopped', 'running'):
            record = {'Name': 'fixture', 'State': state}
            with patch.object(services, 'run', return_value=Mock(stdout=json.dumps([record]))):
                self.assertEqual(services._machine_info('fixture'), record)

    def test_invalid_ssh_config_is_rejected_including_zero_and_boolean_ports(self):
        for config in (None, {}, {'Port': 0, 'RemoteUsername': 'user'},
                       {'Port': True, 'RemoteUsername': 'user'}, {'Port': 65536, 'RemoteUsername': 'user'},
                       {'Port': 12345.0, 'RemoteUsername': 'user'}, {'Port': 12345, 'RemoteUsername': ''},
                       {'Port': 12345, 'RemoteUsername': None}, {'Port': 12345, 'RemoteUsername': 'root'}):
            port = int(config.get('Port', 12345)) if config else 12345
            connections = [{'Name': 'fixture', 'URI': f'ssh://user@127.0.0.1:{port}/run/user/1000/podman/podman.sock'},
                           {'Name': 'fixture-root', 'URI': f'ssh://root@127.0.0.1:{port}/run/podman/podman.sock'}]
            with self.subTest(config=config), patch.object(services, 'run', return_value=Mock(stdout=json.dumps(connections))):
                with self.assertRaises(ValueError):
                    services._machine_connections('fixture', {'SSHConfig': config})

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
            self.assertIs(caught.exception._cleanup_failures[0][1], cleanup)
            self.assertIsNone(caught.exception.__cause__)

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

    def test_rootful_machine_default_is_carried_to_compose_and_inspection(self):
        with patch.object(services, '_machine_info', return_value={'State': 'running', 'Rootful': True}), \
                patch.object(services, '_machine_connections', return_value=['fixture', 'fixture-root']), \
                patch.object(services, 'compose_command', return_value=['podman', 'compose']):
            with services.managed_runtime('podman', 'fixture', environment={}) as runtime:
                self.assertEqual(runtime.environment['CONTAINER_CONNECTION'], 'fixture-root')
                self.assertEqual(runtime.engine, ['podman', '--connection', 'fixture-root'])

    def test_container_host_override_is_rejected_before_machine_start(self):
        with patch.object(services, 'run') as run:
            with self.assertRaises(ValueError):
                with services.managed_runtime('podman', 'fixture', environment={'CONTAINER_HOST': 'ssh://foreign'}):
                    self.fail('foreign host admitted')
            run.assert_not_called()

    def test_connection_uris_reject_queries_fragments_and_passwords(self):
        info = {'SSHConfig': {'Port': 12345, 'RemoteUsername': 'user'}}
        rootless = 'ssh://user@127.0.0.1:12345/run/user/1000/podman/podman.sock'
        rootful = 'ssh://root@127.0.0.1:12345/run/podman/podman.sock'
        for uri in (rootless+'?foo=1', rootless+'#frag', rootless.replace('user@','user:secret@')):
            connections = [{'Name':'fixture','URI':uri},{'Name':'fixture-root','URI':rootful}]
            with self.subTest(uri=uri), patch.object(services,'run',return_value=Mock(stdout=json.dumps(connections))):
                with self.assertRaises(ValueError): services._machine_connections('fixture',info)

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
            run.assert_called_once_with(['podman', 'machine', 'start', 'fixture-machine'], timeout=120, env={'CONTAINER_CONNECTION': 'fixture-machine'}, cwd=services._account_directory())
            stop.assert_called_once_with('fixture-machine', environment={'CONTAINER_CONNECTION': 'fixture-machine'}, stop_seconds=120)

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
            stop.assert_called_once_with('fixture-machine', environment={'CONTAINER_CONNECTION': 'fixture-machine'}, stop_seconds=120)

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
        self.assertTrue(all(call.kwargs['cwd'] == services._account_directory()
                            for call in run.call_args_list))

    def test_invalid_machine_or_engine_combination_is_rejected_before_io(self):
        with patch.object(services, 'run') as run:
            for engine, machine in (('docker', 'fixture'), ('podman', '../fixture')):
                with self.subTest(engine=engine), self.assertRaises(ValueError):
                    with services.managed_runtime(engine, machine):
                        pass
            run.assert_not_called()


class MachineLeaseTests(unittest.TestCase):
    def test_a_second_process_cannot_bypass_the_account_lease_with_environment_aliases(self):
        code = """
import sys
from pathlib import Path
import test_services as services
services._account_directory = lambda: Path(sys.argv[1])
try:
    with services._machine_lease('fixture'):
        sys.exit(1)
except RuntimeError as error:
    print(error)
"""
        with tempfile.TemporaryDirectory() as folder, \
                patch.object(services, '_account_directory', return_value=Path(folder)):
            environment = dict(os.environ, LOGNAME='different-alias', TEMP='different-temp',
                               PYTHONPATH=str(Path(__file__).resolve().parents[1] / 'scripts'))
            with services._machine_lease('fixture'):
                result = subprocess.run([sys.executable, '-c', code, folder], env=environment,
                                        capture_output=True, text=True, timeout=10)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn('Another verifier owns', result.stdout)

    def test_machine_lease_ignores_user_and_temporary_environment_overrides(self):
        first = services._resource_lease_path('fixture')
        # Old and new verifiers must contend on the same existing lock file.
        self.assertEqual(first.name, 'machine-' + hashlib.sha256(b'fixture').hexdigest() + '.lock')
        overrides = dict.fromkeys(
            ('LOGNAME', 'USER', 'USERNAME', 'TEMP', 'TMP', 'TMPDIR', 'HOME',
             'USERPROFILE', 'LOCALAPPDATA'), 'unrelated-environment-value')
        with patch.dict(os.environ, overrides):
            self.assertEqual(first, services._resource_lease_path('fixture'))
        self.assertNotEqual(first, services._resource_lease_path('other-machine'))

    def test_same_machine_name_has_distinct_leases_for_distinct_account_directories(self):
        with tempfile.TemporaryDirectory() as folder:
            for account in ('first', 'second'):
                with patch.object(services, '_account_directory', return_value=Path(folder) / account):
                    path = services._resource_lease_path('fixture')
                if account == 'first':
                    first = path
                else:
                    self.assertNotEqual(first, path)

    def test_lease_excludes_a_second_verifier_and_releases_after_failure(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(services, '_account_directory', return_value=Path(folder)):
            with self.assertRaisesRegex(RuntimeError, 'body failed'):
                with services._machine_lease('fixture'):
                    with self.assertRaisesRegex(RuntimeError, 'Another verifier'):
                        with services._machine_lease('fixture'):
                            self.fail('overlapping verifier admitted')
                    raise RuntimeError('body failed')
            with services._machine_lease('fixture'):
                pass
