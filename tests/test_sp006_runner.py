"""Cheap CI coverage of SP-006 imports, admission and database ownership."""
from contextlib import nullcontext
import importlib.util
import io
import os
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('sp006_runner', ROOT / 'experiments/SP-006/run.py')
runner = importlib.util.module_from_spec(spec)
with patch.object(sys, 'path', [str(ROOT / 'experiments/SP-006'), *sys.path]), \
        patch.dict(sys.modules), patch.object(sys, 'dont_write_bytecode', True):
    # The standalone import must not reuse another test's generic module name.
    sys.modules.pop('protocol', None)
    spec.loader.exec_module(runner)


class RunnerBoundaryTests(unittest.TestCase):
    def test_loader_restores_paths_module_bindings_and_bytecode_policy(self):
        occupied = types.ModuleType('protocol')
        before_path = list(sys.path)
        before_bytecode = sys.dont_write_bytecode
        with patch.dict(sys.modules, {'protocol': occupied}):
            loaded = runpy.run_path(__file__, run_name='sp006_loader_probe')
            self.assertIs(sys.modules['protocol'], occupied)
            self.assertEqual(sys.path, before_path)
            self.assertEqual(sys.dont_write_bytecode, before_bytecode)
            self.assertEqual(loaded['runner'].Rebuild.__module__, 'protocol')

    def test_guarded_standalone_import_has_no_external_work(self):
        script = '''
from pathlib import Path
import os, runpy, sys
sys.path.insert(0, str(Path(sys.argv[1]).parent))
def audit(event, args):
    if event == 'open' and args[2] & (os.O_WRONLY | os.O_RDWR | os.O_CREAT | os.O_TRUNC | os.O_APPEND):
        raise AssertionError('import attempted a write-mode open')
    if event in ('subprocess.Popen', 'os.system', 'os.mkdir', 'os.remove', 'os.rename',
                 'socket.connect', 'socket.bind'):
        raise AssertionError('import attempted external work: ' + event)
sys.addaudithook(audit)
loaded = runpy.run_path(sys.argv[1], run_name='sp006_import')
assert callable(loaded['main'])
assert loaded['Rebuild'].__module__ == 'protocol'
assert callable(sys.modules['protocol'].Search.publish)
'''
        result = subprocess.run([sys.executable, '-B', '-c', script, str(runner.SOURCE / 'run.py')],
                                capture_output=True, text=True, timeout=15)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, '')

    def test_missing_exclusive_mode_is_rejected_before_preflight_or_startup(self):
        with patch.object(sys, 'argv', ['run.py', '--podman-machine', 'fixture']), \
                patch.object(runner, 'preflight') as preflight, \
                patch.object(runner, 'managed_runtime',
                             side_effect=AssertionError('startup was attempted')) as runtime:
            with self.assertRaisesRegex(ValueError, 'exclusive'):
                runner.main()
        preflight.assert_not_called()
        runtime.assert_not_called()

    def test_invalid_temporary_roots_are_rejected_before_preflight_or_startup(self):
        external = str(Path(tempfile.gettempdir()).resolve())
        profiles = [(str(ROOT), {'TEMP': str(ROOT), 'TMP': str(ROOT)}),
                    (external, {'TEMP': external, 'TMP': ''}),
                    (external, {'TEMP': external, 'TMP': str(ROOT)})]
        for actual, environment in profiles:
            with self.subTest(actual=actual, environment=environment), \
                    patch.object(sys, 'argv', ['run.py', '--podman-machine', 'fixture',
                                              '--exclusive-podman-machine']), \
                    patch.object(runner.tempfile, 'gettempdir', return_value=actual), \
                    patch.dict(os.environ, environment), \
                    patch.object(runner, 'preflight') as preflight, \
                    patch.object(runner, 'managed_runtime') as runtime:
                with self.assertRaises(ValueError):
                    runner.main()
                preflight.assert_not_called()
                runtime.assert_not_called()

    def test_disappearing_owned_database_fails_without_global_rediscovery(self):
        external = str(Path(tempfile.gettempdir()).resolve())
        runtime = Mock(engine=['podman', '--connection', 'fixture'], environment={})
        database = Mock()
        database.reset.side_effect = ValueError('owned container exited')
        with patch.object(sys, 'argv', ['run.py', '--podman-machine', 'fixture',
                                      '--exclusive-podman-machine']), \
                patch.dict(os.environ, {'TEMP': external, 'TMP': external}), \
                patch.object(runner, 'preflight'), \
                patch.object(runner, 'work_budget', return_value=nullcontext()), \
                patch.object(runner, 'managed_runtime', return_value=nullcontext(runtime)), \
                patch.object(runner, 'service_group',
                             return_value=nullcontext({'postgres': 'owned-postgres'})), \
                patch.object(runner.subprocess, 'run'), \
                patch.object(runner.subprocess, 'check_output',
                             side_effect=AssertionError('unowned global inventory queried')), \
                patch.object(runner, 'Database', return_value=database) as constructor, \
                patch('sys.stdout', new_callable=io.StringIO) as output:
            with self.assertRaisesRegex(ValueError, 'owned container exited'):
                runner.main()
        constructor.assert_called_once_with(runtime, 'owned-postgres')
        database.reset.assert_called_once_with()
        self.assertEqual(output.getvalue(), '')

    def test_failed_healthcheck_stops_before_database_or_protocol_work(self):
        external = str(Path(tempfile.gettempdir()).resolve())
        runtime = Mock(environment=os.environ.copy())
        actual_run = subprocess.run

        def failed_healthcheck(command, **options):
            self.assertIn('healthcheck.py', command[1])
            return actual_run([sys.executable, '-c', 'raise SystemExit(3)'],
                              check=options.get('check', False), capture_output=True, timeout=5)

        with patch.object(sys, 'argv', ['run.py', '--podman-machine', 'fixture',
                                      '--exclusive-podman-machine']), \
                patch.dict(os.environ, {'TEMP': external, 'TMP': external}), \
                patch.object(runner, 'preflight'), \
                patch.object(runner, 'work_budget', return_value=nullcontext()), \
                patch.object(runner, 'managed_runtime', return_value=nullcontext(runtime)), \
                patch.object(runner, 'service_group', return_value=nullcontext({'postgres': 'owned-postgres'})), \
                patch.object(runner.subprocess, 'run', side_effect=failed_healthcheck), \
                patch.object(runner, 'Database') as database, \
                patch.object(runner, 'sequence_counterexample', return_value={'passed': True}), \
                patch.object(runner, 'live_rebuild', return_value={'passed': True}), \
                patch.object(runner, 'alias_recovery', return_value={'passed': True}), \
                patch.object(runner, 'snapshot_loss', return_value={'passed': True}), \
                patch('sys.stdout', new_callable=io.StringIO) as output:
            with self.assertRaises(subprocess.CalledProcessError):
                runner.main()
        database.assert_not_called()
        self.assertEqual(output.getvalue(), '')


if __name__ == '__main__':
    unittest.main()
