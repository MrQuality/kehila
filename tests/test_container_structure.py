"""Fresh-cluster ownership, isolation, failure and cleanup controls."""
from pathlib import Path
import json
import tempfile
import sys
from types import SimpleNamespace
import unittest
from itertools import chain, repeat
from unittest.mock import Mock, patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import container_structure as structure


class StructuralLifetime(unittest.TestCase):
    def test_diagnostics_inside_source_are_refused_without_creating_a_directory(self):
        executor, _ = self.fixture()
        executor.inspect_container.side_effect = None
        executor.inspect_container.return_value = {
            'Id': '1' * 64, 'Name': 'owned',
            'Config': {'Labels': {structure.LABEL: executor.owner}}}
        error = ValueError('original SQL failure')
        for parent in (executor.source, executor.source / 'nested'):
            with self.subTest(parent=parent), \
                    patch.object(structure.tempfile, 'gettempdir', return_value=str(parent)), \
                    patch.object(structure.tempfile, 'mkdtemp') as create:
                structure.retain_failure(executor, '1' * 64, 'owned', 'foundation', 'sql', error)
                create.assert_not_called()
        self.assertEqual(str(error), 'original SQL failure')
        self.assertEqual(len(error._cleanup_failures), 2)
        self.assertTrue(all('outside source' in str(failure)
                            for _, failure in error._cleanup_failures))

    def test_readiness_deadline_rejects_and_releases_before_sql(self):
        for ticks in ([0, 61], [0, 0, 61], [0, 0, 0, 1, 61]):
            executor, database = self.fixture()
            engine = executor.engine.side_effect

            def bounded_engine(*arguments, **kwargs):
                if 'timeout' in kwargs:
                    self.assertGreater(kwargs['timeout'], 0, 'non-positive readiness inspect timeout')
                return engine(*arguments, **kwargs)

            def probe(*arguments, **kwargs):
                if 'pg_isready' in arguments:
                    self.assertGreater(kwargs['timeout'], 0, 'non-positive readiness probe timeout')
                return SimpleNamespace(returncode=1)

            executor.engine.side_effect = bounded_engine
            database.command.side_effect = probe
            with self.subTest(ticks=ticks), \
                    patch.object(structure, 'EngineDatabase', return_value=database), \
                    patch.object(structure, 'validate_container'), \
                    patch.object(structure, 'retain_failure'), \
                    patch.object(structure, 'command_timeout', side_effect=lambda limit: limit), \
                    patch.object(structure.time, 'monotonic', side_effect=chain(ticks, repeat(ticks[-1]))), \
                    patch.object(structure.time, 'sleep'):
                with self.assertRaisesRegex(ValueError, 'readiness deadline exceeded'):
                    structure.check(executor)
            database.structural_integrity.assert_not_called()
            executor.engine.assert_any_call('rm', '-f', '1' * 64)

    def fixture(self):
        executor = Mock(owner='a' * 32, source=Path('/exported/source'))
        identities = iter(('1' * 64, '2' * 64))
        names = {}

        def engine(*arguments, **kwargs):
            if arguments[0] == 'create':
                identity = next(identities)
                names[identity] = arguments[arguments.index('--name') + 1]
                return SimpleNamespace(stdout=identity)
            if arguments[0] == 'inspect':
                return SimpleNamespace(stdout=json.dumps([executor.inspect_container(arguments[-1])]))
            return SimpleNamespace(stdout='', stderr='')

        executor.engine.side_effect = engine
        executor.inspect_container.side_effect = lambda identity: {
            'Id': identity, 'Name': names[identity],
            'Config': {'Labels': {structure.LABEL: executor.owner}}}
        database = Mock()
        database.command.side_effect = lambda *args, **kwargs: SimpleNamespace(
            returncode=1 if args[0] == 'container' else 0)
        return executor, database

    def test_two_fresh_clusters_composed_product_once_and_verified_release(self):
        executor, database = self.fixture()
        with patch.object(structure, 'EngineDatabase', return_value=database) as bind, \
                patch.object(structure, 'validate_container') as admission, \
                patch.object(structure, 'selection_structure') as selection:
            structure.check(executor)
        self.assertEqual([call.args[1] for call in bind.call_args_list], ['1' * 64, '2' * 64])
        database.structural_integrity.assert_called_once_with()
        selection.assert_called_once_with(database, executor.source)
        self.assertEqual(admission.call_count, 2)
        creations = [call.args for call in executor.engine.call_args_list if call.args[0] == 'create']
        self.assertTrue(all('--network=none' in args and '--pull=never' in args for args in creations))
        self.assertEqual([call.args for call in executor.engine.call_args_list if call.args[0] == 'rm'],
                         [('rm', '-f', '1' * 64), ('rm', '-f', '2' * 64)])
        self.assertEqual(database.command.call_args_list[-1].args,
                         ('container', 'exists', '2' * 64))
        readiness = [call for call in database.command.call_args_list if 'pg_isready' in call.args]
        self.assertTrue(all(call.args[3:5] == ('-h', '127.0.0.1') for call in readiness))

    def test_sql_failure_releases_owned_cluster_and_stops_before_product(self):
        executor, database = self.fixture()
        database.structural_integrity.side_effect = ValueError('deliberate SQL rejection absent')
        with patch.object(structure, 'EngineDatabase', return_value=database), \
                patch.object(structure, 'validate_container'), \
                patch.object(structure, 'selection_structure') as selection:
            with self.assertRaisesRegex(ValueError, 'deliberate SQL rejection absent'):
                structure.check(executor)
        selection.assert_not_called()
        executor.engine.assert_any_call('rm', '-f', '1' * 64)

    def test_foreign_identity_refuses_database_work_and_removal(self):
        executor, database = self.fixture()
        executor.inspect_container.side_effect = None
        executor.inspect_container.return_value = {'Id': 'f' * 64, 'Name': 'foreign', 'Config': {'Labels': {}}}
        with patch.object(structure, 'EngineDatabase', return_value=database):
            with self.assertRaisesRegex(ValueError, 'ownership mismatch'):
                structure.check(executor)
        database.structural_integrity.assert_not_called()
        self.assertFalse(any(call.args[0] in {'rm', 'start'} for call in executor.engine.call_args_list))

    def test_unconfirmed_removal_is_not_a_pass(self):
        executor, database = self.fixture()
        database.command.side_effect = None
        database.command.return_value = SimpleNamespace(returncode=0)
        with patch.object(structure, 'EngineDatabase', return_value=database), \
                patch.object(structure, 'validate_container'):
            with self.assertRaisesRegex(ValueError, 'removal not confirmed'):
                structure.check(executor)

    def test_initialization_cannot_admit_sql_before_loopback_is_ready(self):
        executor, database = self.fixture()
        calls = []

        def probe(*arguments, **kwargs):
            if 'pg_isready' in arguments:
                calls.append('probe')
                self.assertIn('127.0.0.1', arguments)
                return SimpleNamespace(returncode=1 if len(calls) == 1 else 0)
            return SimpleNamespace(returncode=1)

        database.command.side_effect = probe
        database.structural_integrity.side_effect = lambda: self.assertEqual(len(calls), 2)
        with patch.object(structure, 'EngineDatabase', return_value=database), \
                patch.object(structure, 'validate_container'), \
                patch.object(structure, 'selection_structure'), patch.object(structure.time, 'sleep'):
            structure.check(executor)

    def test_failed_startup_retains_diagnostics_before_removal(self):
        executor, database = self.fixture()
        engine = executor.engine.side_effect
        events = []

        def fail_start(*arguments, **kwargs):
            events.append(arguments[0])
            if arguments[0] == 'start':
                raise ValueError('startup failed')
            return engine(*arguments, **kwargs)

        executor.engine.side_effect = fail_start
        with tempfile.TemporaryDirectory() as temporary, \
                patch.object(structure.tempfile, 'gettempdir', return_value=temporary), \
                patch.object(structure, 'EngineDatabase', return_value=database):
            with self.assertRaisesRegex(ValueError, 'startup failed'):
                structure.check(executor)
            records = list(Path(temporary).glob('*/failure.json'))
            self.assertEqual(len(records), 1)
            self.assertEqual(json.loads(records[0].read_text())['stage'], 'startup')
        self.assertLess(events.index('logs'), events.index('rm'))

    def test_failed_diagnostic_collection_preserves_primary_and_cleanup(self):
        executor, database = self.fixture()
        engine = executor.engine.side_effect

        def fail(*arguments, **kwargs):
            if arguments[0] in {'start', 'logs'}:
                raise ValueError(arguments[0] + ' failed')
            return engine(*arguments, **kwargs)

        executor.engine.side_effect = fail
        with patch.object(structure, 'EngineDatabase', return_value=database):
            with self.assertRaisesRegex(ValueError, 'start failed'):
                structure.check(executor)
        executor.engine.assert_any_call('rm', '-f', '1' * 64)


if __name__ == '__main__':
    unittest.main()
