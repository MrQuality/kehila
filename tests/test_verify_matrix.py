"""The documented pure command must execute every pure Rust crate."""
import importlib.util
from contextlib import contextmanager, nullcontext
import os
import io
from pathlib import Path
import subprocess
import sys
import unittest
from unittest.mock import patch
from matrix_fixtures import DEFAULT_ENVIRONMENT

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('verify', ROOT / 'scripts/verify.py')
VERIFY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(VERIFY)


class MatrixTests(unittest.TestCase):
    def setUp(self):
        environment = patch.dict(os.environ, DEFAULT_ENVIRONMENT)
        environment.start()
        self.addCleanup(environment.stop)


class PureMatrixTests(MatrixTests):
    def test_task_contract_failure_fails_the_pure_command(self):
        seen = []

        def run(command, **kwargs):
            seen.append(command)
            if command[0] == 'cargo' and 'task_contract' in command:
                raise subprocess.CalledProcessError(1, command)

        with patch.object(sys, 'argv', ['verify.py', '--pure']), patch.object(VERIFY.subprocess, 'run', run):
            with self.assertRaises(subprocess.CalledProcessError):
                VERIFY.main()
        self.assertTrue(any('task_contract' in command for command in seen))

    def test_production_codec_failure_fails_the_pure_command(self):
        def run(command, **kwargs):
            if command[0] == 'cargo' and 'project_create_codec' in command:
                raise subprocess.CalledProcessError(1, command)
        with patch.object(sys, 'argv', ['verify.py', '--pure']), patch.object(VERIFY.subprocess, 'run', run):
            with self.assertRaises(subprocess.CalledProcessError):
                VERIFY.main()


class FullLifecycleTests(MatrixTests):
    def test_matrix_fixtures_are_independent_of_caller_routing_settings(self):
        from test_engineering import RegisterTests
        fixtures = (FullLifecycleTests('test_all_builds_precede_owned_phases_and_all_live_checks_remain'),
                    RegisterTests('test_shared_matrix_includes_enforcement_in_both_modes'))
        with patch.dict(os.environ, KEHILA_NATS_HOST='standalone-host',
                        DOCKER_HOST='ssh://standalone-daemon'), \
                patch('sys.stdout', new_callable=io.StringIO), \
                patch('sys.stderr', new_callable=io.StringIO):
            for fixture in fixtures:
                result = unittest.TestResult()
                fixture.run(result)
                self.assertTrue(result.wasSuccessful(), result.errors + result.failures)

    def test_owned_mode_rejects_foreign_test_endpoints_before_any_build(self):
        for key, value in (('KEHILA_TEST_MONGO_URL', 'mongodb://other-host:27017'),
                           ('KEHILA_NATS_ADDRESS', '127.0.0.1:4223'),
                           ('KEHILA_NATS_HOST', 'other-host'), ('KEHILA_NATS_PORT', '4223')):
            with self.subTest(key=key), \
                    patch.dict(os.environ, {key: value, 'KEHILA_TEST_ENGINE': 'auto',
                                           'KEHILA_TEST_PODMAN_MACHINE': '',
                                           'KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE': '0'}), \
                    patch.object(sys, 'argv', ['verify.py']), \
                    patch.object(VERIFY, 'run_checks') as checks, \
                    patch.object(VERIFY.subprocess, 'run') as run, \
                    patch('sys.stderr', new_callable=io.StringIO):
                with self.assertRaises(SystemExit):
                    VERIFY.main()
                run.assert_not_called()
                checks.assert_not_called()

    def test_owned_daemon_overrides_are_rejected_before_builds(self):
        for variable in ('DOCKER_HOST','DOCKER_CONTEXT','CONTAINER_HOST','CONTAINER_CONNECTION'):
            with self.subTest(variable=variable), \
                    patch.dict(os.environ,{variable:'foreign','KEHILA_TEST_ENGINE':'docker',
                        'KEHILA_TEST_PODMAN_MACHINE':'','KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE':'0'}), \
                    patch.object(sys,'argv',['verify.py']), \
                    patch.object(VERIFY,'run_checks') as checks, \
                    patch('sys.stderr',new_callable=io.StringIO):
                with self.assertRaises(SystemExit): VERIFY.main()
                checks.assert_not_called()

    def test_external_mode_rejects_endpoints_that_disagree_with_local_readiness(self):
        for key,value in (('KEHILA_TEST_MONGO_URL','mongodb://127.0.0.1:27018'),
                          ('KEHILA_NATS_ADDRESS','127.0.0.1:4223'),
                          ('KEHILA_NATS_HOST','other-host'),('KEHILA_NATS_PORT','4223')):
            with self.subTest(key=key), \
                    patch.dict(os.environ, {key:value,'KEHILA_TEST_ENGINE':'auto',
                        'KEHILA_TEST_PODMAN_MACHINE':'','KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE':'0'}), \
                    patch.object(sys,'argv',['verify.py','--external-services']), \
                    patch.object(VERIFY,'run_checks') as checks, \
                    patch('sys.stderr',new_callable=io.StringIO):
                with self.assertRaises(SystemExit): VERIFY.main()
                checks.assert_not_called()

    def test_external_mode_executes_every_live_check_without_resource_ownership(self):
        folder = Path('unused-fixture')
        with patch.dict(os.environ, KEHILA_TEST_ENGINE='auto', KEHILA_TEST_PODMAN_MACHINE='',
                        KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE='0'), \
                patch.object(sys, 'argv', ['verify.py', '--external-services']), \
                patch.object(VERIFY, 'owned_directory', return_value=nullcontext(folder)), \
                patch.object(VERIFY.subprocess, 'run') as run, \
                patch.object(VERIFY, 'managed_runtime') as runtime:
            VERIFY.main()
        runtime.assert_not_called()
        commands = [call.args[0] for call in run.call_args_list]
        self.assertEqual(commands[-4:], [
            [sys.executable, 'scripts/healthcheck.py', '--wait', '180', '--services',
             'PostgreSQL', 'FerretDB', 'NATS', 'OpenSearch'],
            [sys.executable, 'tests/integration/nats_probe.py'],
            ['cargo', 'test', '--locked', '-p', 'event_dispatcher'],
            [sys.executable, 'tests/integration/task_path.py', '--run-only', str(folder)]])
        self.assertIn(['cargo', 'test', '--locked', '--workspace', '--exclude', 'event_dispatcher'], commands)

    def test_invalid_runtime_settings_fail_before_any_build_or_service(self):
        invalid = [['--engine', 'docker', '--podman-machine', 'fixture'],
                   ['--podman-machine', '../invalid'], ['--podman-machine', ''],
                   ['--external-services', '--podman-machine', 'fixture'],
                   ['--external-services', '--pure'], ['--external-services', '--docs'],
                   ['--exclusive-podman-machine'], ['--timeout', '0'],
                   ['--timeout', 'nan'], ['--timeout', '601']]
        for arguments in invalid:
            with self.subTest(arguments=arguments), \
                    patch.dict(os.environ, KEHILA_TEST_ENGINE='auto', KEHILA_TEST_PODMAN_MACHINE='',
                               KEHILA_TEST_EXCLUSIVE_PODMAN_MACHINE='0'), \
                    patch.object(sys, 'argv', ['verify.py', *arguments]), \
                    patch.object(VERIFY.subprocess, 'run') as run, \
                    patch('sys.stderr', new_callable=io.StringIO):
                with self.assertRaises(SystemExit) as caught:
                    VERIFY.main()
                self.assertEqual(caught.exception.code, 2)
                run.assert_not_called()

    def test_all_builds_precede_owned_phases_and_all_live_checks_remain(self):
        events = []
        environments = []

        @contextmanager
        def runtime(*args, **kwargs):
            events.append('VM start')
            yield ['podman', 'compose']
            events.append('VM stop')

        @contextmanager
        def group(compose, selected):
            events.append(('start', tuple(selected)))
            yield
            events.append(('stop', tuple(selected)))

        def run(command, **kwargs):
            events.append(command)
            environments.append(kwargs['env'])

        with patch.object(sys, 'argv', ['verify.py']), \
                patch.dict(os.environ, CARGO_BUILD_JOBS='8', GOFLAGS='-tags=fixture'), \
                patch.object(VERIFY, 'managed_runtime', runtime), \
                patch.object(VERIFY, 'service_group', group), \
                patch.object(VERIFY.subprocess, 'run', run):
            VERIFY.main()
        start = events.index('VM start')
        self.assertIn(['cargo', 'test', '--locked', '--workspace', '--no-run'], events[:start])
        self.assertIn(['cargo', 'test', '--locked', '--workspace', '--exclude', 'event_dispatcher'], events[:start])
        self.assertTrue(any(isinstance(event, list) and '--build-only' in event for event in events[:start]))
        self.assertFalse(any(isinstance(event, list) and '--build-only' in event for event in events[start:]))
        lifecycle = [event for event in events if isinstance(event, tuple)]
        self.assertEqual(lifecycle, [
            ('start', ('nats',)), ('stop', ('nats',)),
            ('start', ('opensearch',)), ('stop', ('opensearch',)),
            ('start', ('postgres', 'ferretdb')), ('stop', ('postgres', 'ferretdb'))])
        commands = [event for event in events if isinstance(event, list)]
        readiness = [command[command.index('--services') + 1:] for command in commands if '--services' in command]
        self.assertEqual(readiness, [['NATS'], ['OpenSearch'], ['PostgreSQL', 'FerretDB']])
        self.assertIn([sys.executable, 'tests/integration/nats_probe.py'], commands)
        test = ['cargo', 'test', '--locked', '-p', 'event_dispatcher']
        self.assertLess(events.index(('start', ('nats',))), events.index(test))
        self.assertLess(events.index(test), events.index(('stop', ('nats',))))
        self.assertTrue(any('--run-only' in command for command in commands))
        self.assertEqual(events[-1], 'VM stop')
        self.assertTrue(all(env['CARGO_BUILD_JOBS'] == '1' and env['GOFLAGS'] == '-tags=fixture -p=1'
                            for env in environments))

    def test_build_failure_never_starts_vm_or_services(self):
        def run(command, **kwargs):
            if '--no-run' in command:
                raise subprocess.CalledProcessError(1, command)

        with patch.object(sys, 'argv', ['verify.py']), \
                patch.object(VERIFY.subprocess, 'run', run), \
                patch.object(VERIFY, 'managed_runtime') as runtime:
            with self.assertRaises(subprocess.CalledProcessError):
                VERIFY.main()
            runtime.assert_not_called()

    def test_failed_live_check_releases_group_and_runtime_before_propagating(self):
        events = []

        @contextmanager
        def runtime(*args, **kwargs):
            try:
                yield ['docker', 'compose']
            finally:
                events.append('VM released')

        @contextmanager
        def group(compose, selected):
            events.append(tuple(selected))
            try:
                yield
            finally:
                events.append('group released')

        def run(command, **kwargs):
            if 'tests/integration/nats_probe.py' in command:
                raise subprocess.TimeoutExpired(command, 300)

        with patch.object(sys, 'argv', ['verify.py']), \
                patch.object(VERIFY, 'managed_runtime', runtime), \
                patch.object(VERIFY, 'service_group', group), \
                patch.object(VERIFY.subprocess, 'run', run):
            with self.assertRaises(subprocess.TimeoutExpired):
                VERIFY.main()
        self.assertEqual(events, [('nats',), 'group released', 'VM released'])


class OwnerDeadlineTests(unittest.TestCase):
    def test_each_resource_owner_receives_deadline_without_a_parent_kill_timer(self):
        import verification_budget as budget
        with budget.work_budget(10), patch.object(VERIFY.subprocess,'run') as run:
            for script in ('scripts/go_test.py','experiments/SP-002/check_seed_oracle.py',
                           'experiments/SP-003/check.py','tests/integration/task_path.py'):
                VERIFY.execute([sys.executable,script],{'KEHILA_FIXTURE':'retained'})
        self.assertEqual(run.call_count,4)
        for call in run.call_args_list:
            self.assertIsNone(call.kwargs['timeout'])
            self.assertIn('KEHILA_VERIFICATION_DEADLINE',call.kwargs['env'])
            self.assertEqual(call.kwargs['env']['KEHILA_FIXTURE'],'retained')
