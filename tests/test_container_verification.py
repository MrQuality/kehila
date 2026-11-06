"""Container gate routing/ownership checks; no engine is contacted by these tests."""
from contextlib import contextmanager, nullcontext
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import container_executor as execution
import container_verify as coordinator
import container_source as source
import test_services as services
from test_services import ComposeRuntime
from verification_settings import load

IMAGE = 'sha256:' + 'a' * 64
IDENTITY = 'b' * 64


class CallerOwnershipTests(unittest.TestCase):
    def run_main(self, source, record, owner, mode=('--docs',)):
        runtime = ComposeRuntime(['podman', 'compose'], {})
        arguments = ['container_verify.py', *mode, '--image', IMAGE,
                     '--ownership-file', str(record), '--resource-owner', owner]
        def enter():
            self.assertEqual(json.loads(record.read_text())['owner'], owner)
            return nullcontext()
        with patch.object(sys, 'argv', arguments), patch.object(coordinator, 'ROOT', source), \
                patch.object(coordinator, 'load', return_value=load({})), \
                patch.object(coordinator, 'validate_runtime'), patch.object(coordinator, 'validate_daemon_environment'), \
                patch.object(coordinator, 'managed_runtime', return_value=nullcontext(runtime)), \
                patch.object(execution.ContainerExecutor, 'owned', side_effect=enter), \
                patch.object(execution.ContainerExecutor, 'finish_builds'), \
                patch.object(coordinator, 'checks') as checks:
            coordinator.main()
        return checks.call_args.args[1]

    def test_caller_known_owner_is_recorded_before_lifecycle_entry(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source'
            source.mkdir()
            (source / 'docker-compose.yml').write_text('services: {}', encoding='utf-8')
            record = root / 'record.json'
            self.run_main(source, record, 'a' * 32)
            self.assertEqual(json.loads(record.read_text())['owner'], 'a' * 32)
            self.assertEqual(json.loads(record.read_text())['groups'], [])

    def test_invalid_caller_owner_cannot_create_ownership_record(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source'
            source.mkdir()
            record = root / 'record.json'
            with self.assertRaises(ValueError):
                self.run_main(source, record, 'invalid')
            self.assertFalse(record.exists())

    def test_focused_mode_records_no_unrelated_services_and_normalizes_cli_order(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source'
            source.mkdir()
            (source / 'docker-compose.yml').write_text('services: {}', encoding='utf-8')
            record = root / 'record.json'
            args = self.run_main(source, record, 'a' * 32,
                ('--native-item', 'item-pending', '--native-item', 'item-read'))
            self.assertEqual(args.native_item, ('item-read', 'item-pending'))
            self.assertEqual(json.loads(record.read_text())['groups'], [])

    def test_development_ownership_reserves_no_service_groups(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            source = root / 'source'
            source.mkdir()
            (source / 'docker-compose.yml').write_text('services: {}', encoding='utf-8')
            record = root / 'record.json'
            args = self.run_main(source, record, 'a' * 32, ('--development',))
            self.assertTrue(args.development)
            self.assertEqual(json.loads(record.read_text())['groups'], [])


class ExecutorTests(unittest.TestCase):
    def executor(self):
        value = execution.ContainerExecutor(ComposeRuntime(['podman', 'compose'], {}),
                                            load({}), IMAGE, Path.cwd())
        value.container = IDENTITY
        return value

    def test_service_discovery_preserves_full_identity_for_inspection(self):
        executor = self.executor()
        def discover(command, **kwargs):
            identity = IDENTITY if '--no-trunc' in command else IDENTITY[:12]
            return Mock(stdout=identity)
        with patch.object(services, 'run', side_effect=discover):
            identity = services._containers(executor.runtime, 'kehila-test-fixture')[0]
        executor.engine = Mock(return_value=Mock(stdout=json.dumps([{'Id': IDENTITY}])))
        self.assertEqual(executor.inspect_container(identity)['Id'], IDENTITY)

    def test_image_inspection_normalizes_ids_without_using_manifest_digest(self):
        self.assertEqual(execution.inspected_image_id('a' * 64), IMAGE)
        self.assertEqual(execution.inspected_image_id(IMAGE), IMAGE)
        for value in ('a' * 12, 'image:tag', None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                execution.inspected_image_id(value)

    def test_actual_cgroup_limits_are_checked_before_workloads(self):
        with patch.object(source.Path, 'read_text', side_effect=['2147483648', '512', '100000 100000']):
            source.verify_limits(2147483648, 512, 1)
        for memory, pids, cpu in [('max', '512', '100000 100000'),
                                  ('2147483648', 'max', '100000 100000'),
                                  ('2147483648', '512', 'max 100000')]:
            with self.subTest(memory=memory, pids=pids, cpu=cpu), \
                    patch.object(source.Path, 'read_text', side_effect=[memory, pids, cpu]):
                with self.assertRaises(ValueError):
                    source.verify_limits(2147483648, 512, 1)

    def test_mutable_or_missing_image_is_rejected(self):
        for image in (None, '', 'python:latest', 'sha256:short', 'image@sha256:' + 'a' * 63):
            with self.subTest(image=image), self.assertRaises(ValueError):
                execution.image_reference(image)

    def test_remaining_duration_is_translated_not_absolute_host_clock(self):
        executor = self.executor()
        executor.engine = Mock()
        with patch.object(execution, 'command_timeout', return_value=12):
            executor.execute([sys.executable, 'check.py'], {
                'KEHILA_TEST_MONGO_URL': 'mongodb://10.1.0.2:27017',
                'KEHILA_VERIFICATION_DEADLINE': '9999999', 'SECRET': 'never-forward'})
        args = executor.engine.call_args.args
        self.assertIn('12', args)
        self.assertIn('scripts/container_command.py', args)
        self.assertIn('KEHILA_TEST_MONGO_URL=mongodb://10.1.0.2:27017', args)
        self.assertFalse(any('SECRET' in value or '9999999' in value for value in args))
        self.assertEqual(args[-2:], ('python', 'check.py'))

    def test_failed_command_removes_workloads_and_preserves_cleanup_failure(self):
        executor = self.executor()
        primary = subprocess.CalledProcessError(3, ['engine', 'exec'])
        cleanup = OSError('cleanup failed')
        executor.engine = Mock(side_effect=primary)
        executor.remove = Mock(side_effect=cleanup)
        with patch.object(execution, 'command_timeout', return_value=12):
            with self.assertRaises(subprocess.CalledProcessError) as caught:
                executor.execute(['cargo', 'test'])
        self.assertIs(caught.exception, primary)
        executor.remove.assert_called_once()
        self.assertEqual(primary._cleanup_failures, (('failed verification runner', cleanup),))

    def test_machine_command_allowance_is_still_clamped_by_remaining_work(self):
        executor = self.executor()
        executor.settings = {**executor.settings, 'command_seconds': 900}
        executor.engine = Mock()
        with patch.object(execution, 'command_timeout', return_value=37) as remaining:
            executor.execute(['cargo', 'test'])
        remaining.assert_called_once_with(900)
        self.assertIn('37', executor.engine.call_args.args)

    def test_build_network_detachment_is_idempotent(self):
        executor = self.executor()
        executor.engine = Mock()
        executor.finish_builds()
        executor.finish_builds()
        self.assertEqual(executor.engine.call_count, 1)

    def test_effective_cpu_quota_must_match(self):
        executor = self.executor()
        self.assertTrue(executor.cpu_limit_matches({'NanoCpus': 1000000000}))
        self.assertTrue(executor.cpu_limit_matches({'CpuQuota': 100000, 'CpuPeriod': 100000}))
        self.assertFalse(executor.cpu_limit_matches({'CpuQuota': 200000, 'CpuPeriod': 100000}))
        self.assertFalse(executor.cpu_limit_matches({}))

    def test_foreign_service_network_is_rejected_before_connect(self):
        executor = self.executor()
        executor.inspect_container = Mock(return_value={
            'Config': {'Labels': {'com.docker.compose.project': 'development'}},
            'NetworkSettings': {'Networks': {'development': {'IPAddress': '10.1.0.2'}}}})
        executor.engine = Mock()
        with self.assertRaises(ValueError):
            with executor.service_network({'nats': IDENTITY}):
                self.fail('foreign network admitted')
        executor.engine.assert_not_called()

    def test_disconnect_failure_cannot_replace_live_failure(self):
        executor = self.executor()
        executor.inspect_container = Mock(return_value={
            'Config': {'Labels': {'com.docker.compose.project': 'kehila-test-fixture'}},
            'NetworkSettings': {'Networks': {'fixture': {'IPAddress': '10.1.0.2'}}}})
        network = {'Id': 'c' * 64, 'Labels': {'com.docker.compose.project': 'kehila-test-fixture'}}
        cleanup = OSError('disconnect failed')
        executor.engine = Mock(side_effect=[Mock(stdout=json.dumps([network])), Mock(), cleanup])
        primary = RuntimeError('live assertion failed')
        with self.assertRaises(RuntimeError) as caught:
            with executor.service_network({'nats': IDENTITY}):
                raise primary
        self.assertIs(caught.exception, primary)
        self.assertEqual(primary._cleanup_failures,
                         (('verification service network attachment', cleanup),))


class MatrixTests(unittest.TestCase):
    def test_development_executes_unit_matrix_without_native_or_service_start(self):
        executor = Mock()
        args = SimpleNamespace(docs=False, pure=False, development=True,
                               native_authority=False, native_item=None)
        with patch.object(coordinator, 'native_authority') as native, \
                patch.object(coordinator, 'service_group') as group:
            coordinator.checks(executor, args)
        native.assert_not_called()
        group.assert_not_called()
        commands = [call.args[0] for call in executor.execute.call_args_list]
        self.assertIn(['cargo', 'test', '--locked', '--workspace', '--exclude', 'event_dispatcher'], commands)
        self.assertFalse(any('--build-only' in command or '--release' in command for command in commands))

    def test_all_commands_use_container_executor_and_serial_internal_endpoints(self):
        events = []
        executor = Mock()
        executor.runtime = object()
        executor.execute.side_effect = lambda command, *env: events.append(('command', command, env))
        executor.finish_builds.side_effect = lambda: events.append(('builds-finished',))

        @contextmanager
        def group(runtime, names):
            events.append(('start', tuple(names)))
            yield {name: name for name in names}
            events.append(('stop', tuple(names)))

        @contextmanager
        def network(containers):
            yield {name: '10.1.0.' + str(i + 2) for i, name in enumerate(containers)}

        executor.service_network.side_effect = network
        args = SimpleNamespace(docs=False, pure=False, native_authority=False, native_item=None)
        with patch.object(coordinator, 'service_group', group), \
                patch.object(coordinator, 'native_authority', lambda _: events.append(('native',))):
            coordinator.checks(executor, args)
        lifecycle = [event for event in events if event[0] in {'start', 'stop'}]
        self.assertEqual(lifecycle, [
            ('start', ('nats',)), ('stop', ('nats',)),
            ('start', ('opensearch',)), ('stop', ('opensearch',)),
            ('start', ('postgres', 'ferretdb')), ('stop', ('postgres', 'ferretdb'))])
        commands = [event[1] for event in events if event[0] == 'command']
        self.assertIn(['cargo', 'test', '--locked', '--workspace', '--no-run'], commands)
        native_build = ('command', ['cargo', 'test', '--release', '--offline', '--locked',
            '-p', 'kehila_authority', '--lib', '--no-run'], ())
        self.assertLess(events.index(native_build), events.index(('builds-finished',)))
        self.assertIn(['cargo', 'test', '--locked', '-p', 'event_dispatcher'], commands)
        self.assertTrue(any('--build-only' in command for command in commands))
        self.assertTrue(any('--run-only' in command for command in commands))
        self.assertTrue(any('scripts/go_test.py' in command for command in commands))
        self.assertTrue(any('experiments/SP-002/check_seed_oracle.py' in command for command in commands))
        self.assertTrue(any('experiments/SP-003/check.py' in command for command in commands))
        first_group = next(i for i, event in enumerate(events) if event[0] == 'start')
        self.assertLess(events.index(('builds-finished',)), events.index(('native',)))
        self.assertLess(events.index(('native',)), first_group)
        self.assertTrue(any('--build-only' in event[1] for event in events[:first_group]
                            if event[0] == 'command'))
        readies = [command for command in commands if 'scripts/healthcheck.py' in command]
        self.assertEqual(len(readies), 4)
        self.assertTrue(all('--host' in command and '127.0.0.1' not in command for command in readies))

    def test_native_only_mode_still_compiles_and_disconnects_before_fixture(self):
        events = []
        executor = Mock()
        executor.execute.side_effect = lambda command: events.append(('command', command))
        executor.finish_builds.side_effect = lambda: events.append(('disconnect',))
        executor.verify_source.side_effect = lambda: events.append(('source',))
        with patch.object(coordinator, 'native_authority', lambda _: events.append(('native',))):
            coordinator.checks(executor, SimpleNamespace(native_authority=True, native_item=None,
                                                        docs=False, pure=False))
        self.assertEqual(events, [('command', ['cargo', 'test', '--release', '--offline', '--locked', '-p',
            'kehila_authority', '--lib', '--no-run']), ('disconnect',), ('native',), ('source',)])

    def test_focused_native_mode_compiles_then_delegates_exact_cases_and_verifies_source(self):
        events = []
        executor = Mock()
        executor.execute.side_effect = lambda command: events.append(('compile', command))
        executor.finish_builds.side_effect = lambda: events.append(('disconnect',))
        executor.verify_source.side_effect = lambda: events.append(('source',))
        args = SimpleNamespace(native_authority=False, native_item=('item-read',), docs=False, pure=False)
        with patch.object(coordinator, 'native_authority',
                          lambda _, **kw: events.append(('native', kw))), \
                patch.object(coordinator, 'service_group', side_effect=AssertionError('Unrelated service start')):
            coordinator.checks(executor, args)
        self.assertEqual(events, [('compile', [*coordinator.NATIVE_TEST_COMMAND, '--no-run']),
            ('disconnect',), ('native', {'item_cases': ('item-read',)}), ('source',)])

    def test_bad_cli_selection_fails_before_runtime_validation(self):
        for options in (['--native-item', 'typo'],
                        ['--native-item', 'item-read', '--native-item', 'item-read'],
                        ['--native-item', 'item-read', '--pure']):
            with self.subTest(options=options), patch.object(sys, 'argv', ['container_verify.py', *options]), \
                    patch.object(coordinator, 'validate_runtime') as validate:
                with self.assertRaises(SystemExit) as result:
                    coordinator.main()
                self.assertEqual(result.exception.code, 2)
                validate.assert_not_called()
