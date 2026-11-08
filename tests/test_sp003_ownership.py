"""SP-003 preserves compiler failures and reports bounded oracle diagnostics."""
from contextlib import nullcontext
import importlib.util
import io
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


class ArtifactOwnershipTests(unittest.TestCase):
    """Cargo identity, rather than a stale filename, owns each oracle input."""

    def setUp(self):
        self.names = ('task_contract', 'serde_json', 'sha2')
        self.producer = 'path+file:///source/codec#project_create_codec@0.1.0'
        self.packages = {name: 'registry+test#' + name + '@1.0.0' for name in self.names}
        self.metadata = {'packages': [
            {'id': self.producer, 'name': 'project_create_codec',
             'manifest_path': str(ROOT / 'src/pure/project_create_codec/Cargo.toml')},
            *({'id': package, 'name': name} for name, package in self.packages.items()),
        ], 'resolve': {'nodes': [{'id': self.producer, 'deps': [
            {'name': name, 'pkg': package} for name, package in self.packages.items()
        ]}]}}
        self.messages = [{'reason': 'compiler-artifact', 'package_id': package,
                          'target': {'name': name, 'kind': ['lib']},
                          'filenames': ['/target/deps/lib' + name + '.rlib']}
                         for name, package in self.packages.items()]

    def artifacts(self):
        return BUILD.oracle_libraries(self.metadata, self.messages)

    def test_direct_producer_owns_all_three_exact_libraries(self):
        self.assertEqual(self.artifacts(), {
            name: Path('/target/deps/lib' + name + '.rlib') for name in self.names})

    def test_same_name_from_another_package_cannot_supply_missing_dependency(self):
        self.messages[0]['package_id'] = 'registry+other#task_contract@1.0.0'
        with self.assertRaisesRegex(RuntimeError, 'one library'):
            self.artifacts()

    def test_multiple_artifacts_for_the_selected_package_are_ambiguous(self):
        self.messages.append({**self.messages[0], 'filenames': ['/target/deps/other.rlib']})
        with self.assertRaisesRegex(RuntimeError, 'one library'):
            self.artifacts()

    def test_binary_target_cannot_supply_a_library(self):
        self.messages[0]['target']['kind'] = ['bin']
        with self.assertRaisesRegex(RuntimeError, 'one library'):
            self.artifacts()

    def test_libraries_from_different_dependency_directories_are_rejected(self):
        self.messages[0]['filenames'] = ['/other/deps/libtask_contract.rlib']
        with self.assertRaisesRegex(RuntimeError, 'dependency directory'):
            self.artifacts()

    def test_transitive_dependency_cannot_substitute_for_direct_ownership(self):
        self.metadata['resolve']['nodes'][0]['deps'].pop()
        with self.assertRaisesRegex(RuntimeError, 'direct dependency'):
            self.artifacts()

    def test_producer_identity_must_match_the_owned_manifest(self):
        self.metadata['packages'][0]['manifest_path'] = '/another/Cargo.toml'
        with self.assertRaisesRegex(RuntimeError, 'producer'):
            self.artifacts()


class OracleOwnershipTests(unittest.TestCase):
    def test_compiler_failure_remains_primary_when_build_cleanup_fails(self):
        primary = RuntimeError('rustc failed')
        cleanup = PermissionError('oracle executable locked')
        with tempfile.TemporaryDirectory() as folder:
            parent = Path(folder)
            libraries = {name: parent / (name + '.rlib')
                         for name in ('task_contract', 'serde_json', 'sha2')}
            with patch.object(BUILD, 'command', side_effect=[b'', b'host: native\n', b'{}', b'', primary]), \
                    patch.object(BUILD, 'oracle_libraries', return_value=libraries), \
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
            libraries = {name: parent / (name + '.rlib')
                         for name in ('task_contract', 'serde_json', 'sha2')}
            with patch.object(BUILD, 'command', side_effect=[b'', b'host: native\n', b'{}', b'', b'', b'passed', b'']) as command, \
                    patch.object(BUILD, 'oracle_libraries', return_value=libraries), \
                    patch.object(BUILD, 'owned_directory', return_value=nullcontext(parent)) as owned:
                with BUILD.build() as (_, output):
                    self.assertEqual(output, 'passed')
            owned.assert_called_once_with('run-', parent=parent / 'sp003', retry_seconds=5)
            self.assertEqual(command.call_args_list[2].args[0],
                             ['cargo', 'metadata', '--locked', '--offline', '--format-version=1',
                              '--filter-platform', 'native'])
            self.assertEqual(command.call_args_list[3].args[0],
                             ['cargo', 'build', '--locked', '--offline', '-p', 'project_create_codec',
                              '--lib', '--target', 'native', '--message-format=json'])

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
