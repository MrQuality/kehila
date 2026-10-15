"""SP-003 preserves compiler failures and reports bounded oracle diagnostics."""
from contextlib import nullcontext
import importlib.util
import io
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[1]
SOURCE = ROOT / 'experiments/SP-003'
sys.path.insert(0, str(ROOT / 'scripts'))
import temp_work


def load(name, filename):
    spec = importlib.util.spec_from_file_location(name, SOURCE / filename)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


BUILD = load('sp003_build', 'build.py')
with patch.dict(sys.modules, build=BUILD), patch.object(sys, 'path', [str(SOURCE), *sys.path]):
    RUN = load('sp003_run', 'run.py')
    with patch.dict(sys.modules, run=RUN):
        CHECK = load('sp003_check', 'check.py')


class OracleOwnershipTests(unittest.TestCase):
    def test_compiler_failure_remains_primary_when_build_cleanup_fails(self):
        primary = RuntimeError('rustc failed')
        cleanup = PermissionError('oracle executable locked')
        with tempfile.TemporaryDirectory() as folder:
            parent = Path(folder)
            artifacts = b'\n'.join(json.dumps({
                'reason': 'compiler-artifact', 'target': {'name': name},
                'filenames': [str(parent / (name + '.rlib'))],
            }).encode() for name in ('task_contract', 'serde_json', 'sha2'))
            with patch.object(BUILD, 'command', side_effect=[b'', artifacts, primary]), \
                    patch.object(temp_work.shutil, 'rmtree', side_effect=cleanup), \
                    self.assertRaises(RuntimeError) as caught:
                with BUILD.build():
                    self.fail('compiler failure admitted')
            self.assertIs(caught.exception, primary)
            self.assertEqual(primary._cleanup_failures, (('temporary directory', cleanup),))
            # The failed teardown deliberately retained its directory; the fixture owns it.
            for work in (parent / 'sp003').iterdir():
                shutil.rmtree(work)

    def test_build_uses_shared_ownership_with_its_existing_five_second_bound(self):
        with tempfile.TemporaryDirectory() as folder:
            parent = Path(folder)
            artifacts = b'\n'.join(json.dumps({
                'reason': 'compiler-artifact', 'target': {'name': name},
                'filenames': [str(parent / (name + '.rlib'))],
            }).encode() for name in ('task_contract', 'serde_json', 'sha2'))
            with patch.object(BUILD, 'command', side_effect=[b'', artifacts, b'', b'passed', b'']), \
                    patch.object(BUILD, 'owned_directory', return_value=nullcontext(parent)) as owned:
                with BUILD.build() as (_, output):
                    self.assertEqual(output, 'passed')
            owned.assert_called_once_with('run-', parent=parent / 'sp003', retry_seconds=5)

    def test_shared_check_prints_the_actual_case_failure_before_returning_nonzero(self):
        error = subprocess.TimeoutExpired(['codec_oracle', 'request'], 15)

        def fail(cases):
            def timeout():
                raise error
            cases.run('oracle-timeout', timeout)

        with patch.object(CHECK, 'build', return_value=nullcontext((Path('oracle'), ''))), \
                patch.object(CHECK, 'pure_cases', side_effect=fail), \
                patch('sys.stdout', new_callable=io.StringIO), \
                patch('sys.stderr', new_callable=io.StringIO) as diagnostic:
            self.assertEqual(CHECK.run_main(CHECK.main, 'ERR_SP003_CHECK'), 3)
        self.assertIn('oracle-timeout', diagnostic.getvalue())
        self.assertIn(str(error), diagnostic.getvalue())

    def test_case_diagnostics_are_bounded_utf8_lines(self):
        records = [{'status': 'FAIL', 'case': 'large', 'error': '\n\r' + '字' * 10000}]
        with patch('sys.stderr', new_callable=io.StringIO) as diagnostic:
            CHECK.report_failures(records)
        lines = diagnostic.getvalue().splitlines()
        self.assertEqual(len(lines), 1)
        self.assertLessEqual(len(lines[0].encode('utf-8')), 4096)
        self.assertIn('\\n\\r', lines[0])
