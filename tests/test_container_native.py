"""Admission and owned dependency cleanup, without contacting an engine."""
from pathlib import Path
from types import SimpleNamespace
import json
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import container_native as native
from container_executor import LABEL
from verification_budget import DeadlineExceeded, command_timeout, work_budget

OWNER, RUNNER, DATABASE = 'a' * 32, 'b' * 64, 'c' * 64
NAME = 'kehila-verify-' + OWNER + '-native-db'


def database():
    return {'Id': DATABASE, 'Name': NAME, 'Config': {'Labels': {LABEL: OWNER}, 'Image': native.IMAGE},
            'State': {'Running': True}, 'Mounts': [], 'HostConfig': {'Privileged': False,
            'Binds': [], 'Devices': [], 'PortBindings': {}, 'NetworkMode': 'container:' + RUNNER,
            'Memory': 256 * 1024**2, 'PidsLimit': 128, 'NanoCpus': 10**9,
            'Tmpfs': {'/var/lib/postgresql/data': 'rw,size=256m,rprivate'}}}


def runner():
    return {'Id': RUNNER, 'Config': {'Labels': {LABEL: OWNER}, 'User': '10001:10001'},
            'State': {'Running': True}, 'NetworkSettings': {'Networks': {}},
            'HostConfig': {'NetworkMode': 'bridge', 'PortBindings': {}}}


class AdmissionTests(unittest.TestCase):
    def test_write_trace_requires_each_ordinal_in_exact_order_once(self):
        expected = ['kehila.operations INSERT', 'kehila_auth.pending_projects UPDATE']
        log = ('WARNING: NATIVE_WRITE 1 kehila.operations INSERT\n'
               'WARNING: NATIVE_WRITE 2 kehila_auth.pending_projects UPDATE\n')
        native.verify_write_trace(expected, log)
        for changed in (log.splitlines()[0], log + log, log.replace('WRITE 2', 'WRITE 3'),
                        log.replace('pending_projects', 'creation_intents')):
            with self.assertRaises(ValueError):
                native.verify_write_trace(expected, changed)

    def test_snapshot_comparison_preserves_numeric_tokens_and_boolean_types(self):
        value = native.snapshot_document('{"value":1}')
        for changed in ('{"value":true}', '{"value":1.0}', '{"value":1.0000000000000000001}'):
            self.assertNotEqual(value, native.snapshot_document(changed))
        self.assertNotEqual(native.snapshot_document('{"value":0.12345678901234567890}'),
                            native.snapshot_document('{"value":0.12345678901234567891}'))
        with self.assertRaises(ValueError):
            native.snapshot_document('{"value":NaN}')

    def test_exact_owned_namespace_profile_is_admitted(self):
        native.admit_runner(runner(), RUNNER, OWNER)
        native.admit_database(database(), DATABASE, RUNNER, OWNER, NAME)

    def test_foreign_owner_target_and_network_are_denied(self):
        for identity, parent, owner, name in [('d' * 64, RUNNER, OWNER, NAME),
                (DATABASE, 'e' * 64, OWNER, NAME), (DATABASE, RUNNER, 'f' * 32, NAME),
                (DATABASE, RUNNER, OWNER, NAME + '-different')]:
            with self.assertRaises(ValueError):
                native.admit_database(database(), identity, parent, owner, name)

    def test_mounts_ports_privilege_and_excess_limits_are_denied(self):
        for key, value in [('Memory', 512 * 1024**2), ('PidsLimit', 256), ('NanoCpus', 2 * 10**9),
                ('Privileged', True), ('Binds', ['/host:/data']), ('Devices', ['device']),
                ('PortBindings', {'5432/tcp': [{'HostPort': '5432'}]}), ('NetworkMode', 'host'),
                ('Tmpfs', {'/var/lib/postgresql/data': 'rw,size=512m'})]:
            info = database()
            info['HostConfig'][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                native.admit_database(info, DATABASE, RUNNER, OWNER, NAME)
        info = database()
        info['Mounts'] = [{'Type': 'volume'}]
        with self.assertRaises(ValueError):
            native.admit_database(info, DATABASE, RUNNER, OWNER, NAME)

    def test_attached_and_host_networks_are_denied_before_child_creation(self):
        for mode, attached in [('host', {}), ('bridge', {'build': {}})]:
            info = runner()
            info['HostConfig']['NetworkMode'] = mode
            info['NetworkSettings']['Networks'] = attached
            with self.assertRaises(ValueError):
                native.admit_runner(info, RUNNER, OWNER)


class Executor:
    def __init__(self, fail_journey=False, foreign=False, profile_mismatch=False, copy_mismatch=False,
                 fail_observer=None):
        self.build_network_detached = True
        self.container, self.owner, self.name = RUNNER, OWNER, NAME.removesuffix('-native-db')
        self.settings = {'command_seconds': 300, 'runner_stop_seconds': 120}
        self.source = Path('/public-source')
        self.events, self.fail_journey, self.foreign = [], fail_journey, foreign
        self.profile_mismatch = profile_mismatch
        self.copy_mismatch = copy_mismatch
        self.fail_observer = fail_observer

    def inspect_container(self, identity):
        return runner()

    def engine(self, *args, **kwargs):
        self.events.append(args)
        if args[0] == 'create':
            output = DATABASE
        elif args[0] == 'cp' and args[1].endswith('/controls/write-trace.json'):
            Path(args[2]).write_text('["kehila.operations INSERT"]', encoding='utf-8')
            output = ''
        elif args[0] == 'logs':
            output = 'WARNING: NATIVE_WRITE 1 kehila.operations INSERT\n'
        elif args[0] == 'inspect':
            info = database()
            if self.foreign:
                info['Config']['Labels'][LABEL] = 'd' * 32
            if self.profile_mismatch:
                info['HostConfig']['Memory'] *= 2
            output = json.dumps([info])
        elif args[0] == 'ps':
            output = DATABASE
        elif args[0] == 'exec' and 'journey' in args and self.fail_journey:
            raise subprocess.CalledProcessError(1, args)
        elif args[0] == 'exec' and self.fail_observer and self.fail_observer in args:
            raise subprocess.CalledProcessError(1, args)
        elif args[0] == 'exec' and 'sh' in args:
            output = 'READY'
        elif args[0] == 'exec' and '/tmp/native_copy_snapshot.sql' in args:
            output = json.dumps(dict(tables={'fixture': []}, sequences={'fixture': {}},
                schemas=[], relations=[], types=[], indexes=[], columns=[], functions=[], constraints=[],
                triggers=[], raw_checks=[], mismatch=self.copy_mismatch and 'kehila_native_restored' in args))
        else:
            output = ''
        return SimpleNamespace(stdout=output, stderr='diagnostic stderr' if args[0] == 'logs' else '')


class CleanupTests(unittest.TestCase):
    def setUp(self):
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.root = Path(directory.name)
        temporary = patch.object(native.tempfile, 'gettempdir', return_value=directory.name)
        temporary.start()
        self.addCleanup(temporary.stop)

    def test_dependent_database_is_removed_by_full_identity(self):
        executor = Executor()
        native.check(executor)
        removals = [args for args in executor.events if args[0] == 'rm']
        self.assertEqual(removals, [('rm', '--force', '--volumes', DATABASE)])

    def test_authentication_copies_precede_all_runtime_journeys_and_are_observed(self):
        executor = Executor()
        native.check(executor)
        first_runtime = next(i for i, args in enumerate(executor.events)
                             if args[0] == 'exec' and 'journey' in args)
        for mode, database in native.AUTH_DATABASES.items():
            creations = [i for i, args in enumerate(executor.events)
                         if args[0] == 'exec' and 'createdb' in args and args[-1] == database]
            self.assertEqual(len(creations), 1)
            self.assertLess(creations[0], first_runtime)
            invocations = [i for i, args in enumerate(executor.events)
                           if args[0] == 'exec' and mode in args]
            observations = [i for i, args in enumerate(executor.events)
                            if args[0] == 'exec' and '/tmp/native_auth_fixture_state.sql' in args
                            and database in args]
            self.assertEqual(len(invocations), 1)
            self.assertEqual(len(observations), 1)
            self.assertLess(invocations[0], observations[0])

    def test_item_copies_are_pristine_and_have_distinct_committed_observers(self):
        executor = Executor()
        native.check(executor)
        first_runtime = next(i for i, args in enumerate(executor.events)
                             if args[0] == 'exec' and 'journey' in args)
        for mode, database in native.ITEM_DATABASES.items():
            expected_observer = {'item-read': '/tmp/native_item_read_state.sql',
                                 'item-pending': '/tmp/native_item_pending_state.sql'}[mode]
            creations = [i for i, args in enumerate(executor.events)
                         if args[0] == 'exec' and 'createdb' in args and args[-1] == database]
            invocations = [i for i, args in enumerate(executor.events)
                           if args[0] == 'exec' and mode in args]
            observations = [i for i, args in enumerate(executor.events)
                            if args[0] == 'exec' and expected_observer in args and database in args]
            self.assertEqual(len(creations), 1)
            self.assertEqual(len(invocations), 1)
            self.assertEqual(len(observations), 1)
            self.assertLess(creations[0], first_runtime)
            self.assertLess(invocations[0], observations[0])

    def test_failed_pending_observer_stops_before_later_journeys_and_retains_stage(self):
        executor = Executor(fail_observer='/tmp/native_item_pending_state.sql')
        with self.assertRaisesRegex(RuntimeError,
                'Native fixture command failed at item-pending-independent-observer'):
            native.check(executor)
        self.assertFalse(any(args[0] == 'exec' and 'journey' in args for args in executor.events))
        self.assertIn(('rm', '--force', '--volumes', DATABASE), executor.events)
        folders = list(self.root.glob('kehila-native-diagnostics-*'))
        self.assertEqual(len(folders), 1)
        self.assertEqual((folders[0] / 'failure-stage.txt').read_text(encoding='ascii'),
                         'item-pending-independent-observer')

    def test_child_is_removed_after_failed_work_without_parent_teardown(self):
        executor = Executor(fail_journey=True)
        with self.assertRaises(subprocess.CalledProcessError):
            native.check(executor)
        self.assertIn(('rm', '--force', '--volumes', DATABASE), executor.events)
        self.assertFalse(any(args[0] == 'rm' and RUNNER in args for args in executor.events))
        folders = list(self.root.glob('kehila-native-diagnostics-*'))
        self.assertEqual(len(folders), 1)
        self.assertIn('diagnostic stderr', (folders[0] / 'postgres.log').read_text(encoding='utf-8'))

    def test_owned_rejected_profile_is_removed_without_running_it(self):
        executor = Executor(profile_mismatch=True)
        with self.assertRaises(ValueError):
            native.check(executor)
        self.assertIn(('rm', '--force', '--volumes', DATABASE), executor.events)
        self.assertFalse(any(args[0] == 'start' for args in executor.events))

    def test_different_restored_snapshot_stops_before_restored_authority(self):
        executor = Executor(copy_mismatch=True)
        with self.assertRaisesRegex(ValueError, 'copy state/schema/authority differs'):
            native.check(executor)
        self.assertIn(('rm', '--force', '--volumes', DATABASE), executor.events)
        self.assertFalse(any(args[0] == 'exec' and 'restored' in args for args in executor.events))

    def test_expired_work_budget_still_releases_child_and_preserves_logs(self):
        clock = [0.0]

        class ExpiringExecutor(Executor):
            def engine(self, *args, **kwargs):
                if args[0] == 'exec' and 'journey' in args:
                    clock[0] = 101.0
                command_timeout(1)
                return super().engine(*args, **kwargs)

        executor = ExpiringExecutor()
        with patch('verification_budget.time.monotonic', side_effect=lambda: clock[0]):
            with work_budget(100):
                with self.assertRaises(DeadlineExceeded):
                    native.check(executor)
                # Cleanup did not reopen admission for later ordinary work.
                with self.assertRaises(DeadlineExceeded):
                    command_timeout(1)
        self.assertIn(('rm', '--force', '--volumes', DATABASE), executor.events)
        self.assertFalse(any(args[0] == 'rm' and RUNNER in args for args in executor.events))
        folders = list(self.root.glob('kehila-native-diagnostics-*'))
        self.assertEqual(len(folders), 1)
        self.assertIn('diagnostic stderr', (folders[0] / 'postgres.log').read_text(encoding='utf-8'))

    def test_source_and_git_temporary_storage_are_rejected_before_work(self):
        executor = Executor()
        executor.source = self.root
        with self.assertRaises(ValueError):
            native.check(executor)
        self.assertEqual(executor.events, [])
        executor.source = Path('/public-source')
        (self.root / '.git').mkdir()
        with self.assertRaises(ValueError):
            native.check(executor)
        self.assertEqual(executor.events, [])

    def test_foreign_inventory_cannot_authorize_removal(self):
        executor = Executor(foreign=True)
        with self.assertRaises(ValueError):
            native.check(executor)
        self.assertFalse(any(args[0] == 'rm' for args in executor.events))


if __name__ == '__main__':
    unittest.main()
