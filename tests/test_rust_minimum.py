"""Compiler selection must reflect manifests, not a hardcoded compatibility claim."""
import unittest
import sys
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch
import json

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from check_rust_minimum import compiler_environment, minimum_groups, normalized
import check_rust_minimum as minimum


class MinimumSelection(unittest.TestCase):
    def test_invalid_qualification_inputs_are_rejected_before_any_process(self):
        for home, message in [(Path('relative'), 'absolute'),
                              (minimum.ROOT, 'outside source'),
                              (minimum.ROOT / 'nested', 'outside source')]:
            with self.subTest(home=home), patch.dict(minimum.os.environ, {}, clear=True), \
                    patch.object(minimum.subprocess, 'run',
                                 side_effect=AssertionError('process launched before input admission')) as run:
                with self.assertRaisesRegex(ValueError, message):
                    minimum.main(['--version', '1.75', '--toolchains-home', str(home)])
                run.assert_not_called()
        with patch.dict(minimum.os.environ, {'RUSTC_WRAPPER': 'other'}, clear=True), \
                patch.object(minimum.subprocess, 'run') as run:
            with self.assertRaisesRegex(ValueError, 'override'):
                minimum.main(['--version', '1.75'])
            run.assert_not_called()

    def test_actual_cargo_command_disables_configured_wrappers_and_pins_rustc(self):
        metadata = {'workspace_members': ['model'], 'packages': [
            {'id': 'model', 'name': 'model', 'rust_version': '1.75'}]}
        sysroot = minimum.ROOT.parent / 'qualified-toolchain'
        results = [SimpleNamespace(stdout=json.dumps(metadata)),
                   SimpleNamespace(stdout='rustc 1.75.0 (identity)'),
                   SimpleNamespace(stdout=str(sysroot)), SimpleNamespace(stdout='')]
        with patch.dict(minimum.os.environ, {}, clear=True), \
                patch.object(minimum.Path, 'is_file', return_value=True), \
                patch.object(minimum.subprocess, 'run', side_effect=results) as run:
            minimum.main(['--version', '1.75'])
        command = run.call_args.args[0]
        self.assertEqual(command[:4], ['rustup', 'run', '1.75.0', 'cargo'])
        self.assertIn('build.rustc-wrapper=""', command)
        self.assertIn('build.rustc-workspace-wrapper=""', command)
        self.assertEqual(command[-2:], ['-p', 'model'])
        environment = run.call_args.kwargs['env']
        self.assertEqual(environment['CARGO_NET_OFFLINE'], 'true')
        self.assertEqual(Path(environment['RUSTC']).parent.parent, sysroot)

    def test_compiler_identity_cannot_be_overridden_during_the_cargo_check(self):
        self.assertEqual(compiler_environment({'CARGO_BUILD_JOBS': '1'}, '/qualified/rustc'), {
            'CARGO_BUILD_JOBS': '1', 'CARGO_NET_OFFLINE': 'true', 'RUSTC': '/qualified/rustc'})
        for key in ('RUSTC', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
                    'CARGO_BUILD_RUSTC', 'CARGO_BUILD_RUSTC_WRAPPER',
                    'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER'):
            with self.subTest(key=key), self.assertRaisesRegex(ValueError, 'override'):
                compiler_environment({key: '/other/compiler'}, '/qualified/rustc')

    def test_exact_versions_group_workspace_members_and_exclude_dependencies(self):
        metadata = {'workspace_members': ['a', 'b', 'c'], 'packages': [
            {'id': 'a', 'name': 'pure_a', 'rust_version': '1.75'},
            {'id': 'b', 'name': 'pure_b', 'rust_version': '1.75.0'},
            {'id': 'c', 'name': 'authority', 'rust_version': '1.88'},
            {'id': 'external', 'name': 'dependency', 'rust_version': None},
        ]}
        self.assertEqual(minimum_groups(metadata), {
            '1.75.0': ['pure_a', 'pure_b'], '1.88.0': ['authority']})

    def test_missing_minimum_is_not_a_default_current_compiler_pass(self):
        with self.assertRaises(ValueError):
            minimum_groups({'workspace_members': ['a'], 'packages': [
                {'id': 'a', 'name': 'missing', 'rust_version': None}]})
        with self.assertRaises(ValueError):
            minimum_groups({'workspace_members': [], 'packages': []})
        for value in ('stable', 'nightly', '1.75;command', '1', None):
            with self.subTest(value=value), self.assertRaises(ValueError):
                normalized(value)


if __name__ == '__main__':
    unittest.main()
