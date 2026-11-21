"""Compiler selection must reflect manifests, not a hardcoded compatibility claim."""
import unittest
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from check_rust_minimum import compiler_environment, minimum_groups, normalized


class MinimumSelection(unittest.TestCase):
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
