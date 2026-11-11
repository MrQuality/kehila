"""Resource lifetime for finite and externally controlled native fixtures."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import native_fixture_environment as fixture
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


if __name__ == '__main__':
    unittest.main()
