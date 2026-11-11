"""Resource lifetime for finite and externally controlled native fixtures."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import native_fixture_environment as fixture
from verification_budget import _deadline, command_timeout, work_budget
from test_container_native import Executor, DATABASE, runner


class EnvironmentTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        temporary = patch.object(fixture.tempfile, 'gettempdir', return_value=directory.name)
        temporary.start()
        self.addCleanup(temporary.stop)

    def environment(self, executor, **options):
        return fixture.NativeEnvironment(executor,
            databases=('native_item_mutation',),
            observers=('native_item_mutation_state.sql',),
            selection={'scope': 'browser-recovery', 'selected_items': ['item-mutation'],
                       'observed_items': []}, **options)

    def test_body_retains_database_until_its_observation_is_complete(self):
        executor = Executor()
        environment = self.environment(executor)
        with environment.owned() as active:
            self.assertIs(active, environment)
            self.assertFalse(any(args[0] == 'rm' for args in executor.events))
            active.stage = 'long-lived-independent-observer'
            active.psql('/tmp/native_item_mutation_state.sql', 'native_item_mutation')
            executor.events.append(('body-complete',))
        self.assertLess(executor.events.index(('body-complete',)),
                        executor.events.index(('rm', '--force', '--volumes', DATABASE)))
        self.assertIsNone(environment.receipt)
        self.assertFalse(environment.folder.exists())
        self.assertFalse(any(args[0] == 'remove-parent' for args in executor.events))

    def test_setup_faults_before_yield_still_remove_the_owned_database(self):
        for target in ('start', 'install_product.sql', 'createdb'):
            with self.subTest(target=target):
                class Failure(Executor):
                    def engine(self, *args, **kwargs):
                        result = super().engine(*args, **kwargs)
                        if ((target == 'start' and args[0] == 'start')
                                or (target == 'createdb' and 'createdb' in args)
                                or (target == 'install_product.sql'
                                    and '/tmp/kehila-schema/install_product.sql' in args)):
                            raise subprocess.CalledProcessError(1, args)
                        return result
                executor = Failure()
                environment = self.environment(executor)
                with self.assertRaises(RuntimeError):
                    with environment.owned():
                        self.fail('Failed acquisition yielded a resource')
                self.assertIn(('rm', '--force', '--volumes', DATABASE), executor.events)
                self.assertTrue((environment.folder / 'failure-stage.txt').is_file())

    def test_failure_callback_precedes_removal_and_cannot_replace_primary(self):
        executor = Executor()
        def observe(error, environment):
            self.assertEqual(str(error), 'body failed')
            executor.events.append(('failure-observation', environment.stage))
            raise ValueError('diagnostic failed')
        environment = self.environment(executor, failure_observer=observe)
        with self.assertRaisesRegex(RuntimeError, 'body failed') as caught:
            with environment.owned():
                environment.stage = 'controlled-body'
                raise RuntimeError('body failed')
        self.assertLess(executor.events.index(('failure-observation', 'controlled-body')),
                        executor.events.index(('rm', '--force', '--volumes', DATABASE)))
        self.assertTrue(any(str(error) == 'diagnostic failed'
                            for _, error in caught.exception._cleanup_failures))

    def test_attached_network_or_ports_cannot_enter_the_body(self):
        for field in ('network', 'port'):
            with self.subTest(field=field):
                class Exposed(Executor):
                    def inspect_container(self, identity):
                        value = runner()
                        if field == 'network':
                            value['NetworkSettings']['Networks'] = {'unexpected': {}}
                        else:
                            value['HostConfig']['PortBindings'] = {'16291/tcp': [{}]}
                        return value
                executor = Exposed()
                with self.assertRaises(ValueError):
                    with self.environment(executor).owned():
                        self.fail('Unadmitted exposure yielded a resource')
                self.assertFalse(any(args[0] == 'create' for args in executor.events))

    def test_unknown_database_or_observer_is_rejected_before_creation(self):
        for databases, observers in [(('production',), ('native_item_mutation_state.sql',)),
                                     (('native_item_mutation',), ('../outside.sql',))]:
            executor = Executor()
            with self.assertRaises(ValueError):
                environment = fixture.NativeEnvironment(executor, databases=databases,
                    observers=observers, selection={'scope': 'browser-recovery',
                    'selected_items': ['item-mutation'], 'observed_items': []})
                with environment.owned():
                    self.fail('Unknown fixture selection yielded')
            self.assertEqual(executor.events, [])

    def test_one_environment_cannot_be_reentered_as_a_fresh_fixture(self):
        executor = Executor()
        environment = self.environment(executor)
        with environment.owned():
            pass
        before = list(executor.events)
        with self.assertRaises(ValueError):
            with environment.owned():
                self.fail('Consumed environment yielded again')
        self.assertEqual(executor.events, before)

    def test_dependents_release_once_before_database_cleanup_on_success(self):
        executor = Executor()
        calls = []
        def release(environment, primary):
            calls.append((environment, primary))
            executor.events.append(('dependents-released',))
        environment = self.environment(executor, release_dependents=release)
        with environment.owned():
            executor.events.append(('body-complete',))
        self.assertEqual(calls, [(environment, None)])
        self.assertLess(executor.events.index(('body-complete',)),
                        executor.events.index(('dependents-released',)))
        self.assertLess(executor.events.index(('dependents-released',)),
                        executor.events.index(('rm', '--force', '--volumes', DATABASE)))
        self.assertFalse(environment.cleanup_withheld)

    def test_release_failure_preserves_body_failure_and_withholds_destruction(self):
        executor = Executor()
        calls = []
        def release(environment, primary):
            calls.append(primary)
            raise RuntimeError('dependent join unavailable')
        environment = self.environment(executor, release_dependents=release,
            evidence_directory=self.root / 'exported', reset_assets=True)
        primary = ValueError('body failed')
        cause = RuntimeError('original cause')
        primary.__cause__ = cause
        with self.assertRaises(ValueError) as caught:
            with environment.owned():
                raise primary
        self.assertIs(caught.exception, primary)
        self.assertIs(caught.exception.__cause__, cause)
        self.assertEqual(calls, [primary])
        self.assertTrue(environment.cleanup_withheld)
        self.assertTrue(any(str(error) == 'dependent join unavailable'
                            for _, error in primary._cleanup_failures))
        self.assertFalse(any(args[0] == 'rm' for args in executor.events))
        self.assertFalse(any('reset' in args or 'manifest' in args for args in executor.events))
        self.assertFalse((self.root / 'exported').exists())
        self.assertIsNone(environment.receipt)
        self.assertTrue(environment.folder.exists())

    def test_successful_body_cannot_hide_release_failure(self):
        executor = Executor()
        def release(environment, primary):
            self.assertIsNone(primary)
            raise RuntimeError('dependent release failed')
        environment = self.environment(executor, release_dependents=release)
        with self.assertRaisesRegex(RuntimeError, 'dependent release failed'):
            with environment.owned():
                pass
        self.assertTrue(environment.cleanup_withheld)
        self.assertFalse(any(args[0] == 'rm' for args in executor.events))

    def test_release_runs_after_acquisition_failure_and_with_expired_work_budget(self):
        class Failure(Executor):
            def engine(self, *args, **kwargs):
                result = super().engine(*args, **kwargs)
                if args[0] == 'start':
                    _deadline.set(0)
                    raise subprocess.CalledProcessError(1, args)
                return result
        executor = Failure()
        calls = []
        def release(environment, primary):
            self.assertIsNotNone(primary)
            calls.append(command_timeout(5))
            executor.events.append(('dependents-released',))
        environment = self.environment(executor, release_dependents=release)
        with self.assertRaises(RuntimeError):
            with work_budget(10):
                with environment.owned():
                    self.fail('Acquisition should fail')
        self.assertEqual(calls, [5])
        self.assertLess(executor.events.index(('dependents-released',)),
                        executor.events.index(('rm', '--force', '--volumes', DATABASE)))

    def test_invalid_release_gate_is_rejected_before_resource_acquisition(self):
        executor = Executor()
        with self.assertRaises(ValueError):
            self.environment(executor, release_dependents=True)
        self.assertEqual(executor.events, [])

    def test_partial_acquisition_without_a_folder_preserves_admission_failure(self):
        class Exposed(Executor):
            def inspect_container(self, identity):
                value = runner()
                value['NetworkSettings']['Networks'] = {'unexpected': {}}
                return value
        for release_fails in (False, True):
            with self.subTest(release_fails=release_fails):
                executor = Exposed()
                calls = []
                def release(environment, primary):
                    self.assertIsNone(environment.folder)
                    self.assertIsInstance(primary, ValueError)
                    calls.append(primary)
                    if release_fails:
                        raise RuntimeError('partial dependent release failed')
                environment = self.environment(executor, release_dependents=release)
                with self.assertRaisesRegex(ValueError, 'attached network') as caught:
                    with environment.owned():
                        self.fail('Unadmitted runner yielded')
                self.assertEqual(calls, [caught.exception])
                self.assertEqual(environment.cleanup_withheld, release_fails)
                self.assertFalse(any(args[0] in {'create', 'rm'} for args in executor.events))
                if release_fails:
                    self.assertTrue(any(str(error) == 'partial dependent release failed'
                                        for _, error in caught.exception._cleanup_failures))


if __name__ == '__main__':
    unittest.main()
