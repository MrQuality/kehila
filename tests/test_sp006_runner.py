"""Cheap CI coverage of SP-006 imports, admission and database ownership."""
from contextlib import nullcontext
import importlib.util
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / 'experiments/SP-006'))
spec = importlib.util.spec_from_file_location('sp006_runner', ROOT / 'experiments/SP-006/run.py')
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class RunnerBoundaryTests(unittest.TestCase):
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


if __name__ == '__main__':
    unittest.main()
