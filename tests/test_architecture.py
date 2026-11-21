"""Discriminating dependency inventory controls; compiler fixtures run separately."""
from pathlib import Path
import tempfile
import unittest
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
from check_architecture import check_go, documents, rust_packages


class DependencyBoundaries(unittest.TestCase):
    def test_cargo_actual_names_paths_and_workspace_membership(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'src/pure/model/Cargo.toml'
            manifest.parent.mkdir(parents=True)
            manifest.touch()
            package = {'id': 'model-id', 'name': 'model', 'manifest_path': str(manifest),
                       'dependencies': [{'name': 'serde', 'rename': 'codec'}]}
            metadata = {'packages': [package], 'workspace_members': ['model-id']}
            self.assertEqual(rust_packages(metadata, root), ['model'])
            package['dependencies'] = [{'name': 'tokio', 'rename': 'serde'}]
            with self.assertRaisesRegex(ValueError, 'unreviewed pure dependency tokio'):
                rust_packages(metadata, root)
            package['dependencies'] = [{'name': 'task_contract', 'path': str(root / 'src/io/adapter')}]
            with self.assertRaisesRegex(ValueError, 'leaves the pure layer'):
                rust_packages(metadata, root)
            package['dependencies'] = []
            metadata['workspace_members'] = []
            with self.assertRaisesRegex(ValueError, 'outside the verified workspace'):
                rust_packages(metadata, root)

    def test_missing_crates_are_not_silently_ignored(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            manifest = root / 'src/pure/model/Cargo.toml'
            manifest.parent.mkdir(parents=True)
            manifest.touch()
            with self.assertRaisesRegex(ValueError, 'inventory'):
                rust_packages({'packages': [], 'workspace_members': []}, root)

    def test_go_import_identity_and_test_only_allowance(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package_dir = root / 'go/pure/model'
            package_dir.mkdir(parents=True)
            (package_dir / 'model.go').touch()
            package = {'Dir': str(package_dir), 'ImportPath': 'example/model',
                       'Imports': ['strings'], 'TestImports': ['testing']}
            check_go([package], root)
            for field, name in [('Imports', 'testing'), ('Imports', 'os'),
                                ('Imports', 'fmt'), ('Imports', 'example/adapter'), ('TestImports', 'net')]:
                with self.subTest(field=field, name=name):
                    changed = dict(package, **{field: [name]})
                    with self.assertRaisesRegex(ValueError, 'unreviewed pure import'):
                        check_go([changed], root)
            with self.assertRaisesRegex(ValueError, 'cgo'):
                check_go([dict(package, CgoFiles=['native.go'])], root)
            with self.assertRaisesRegex(ValueError, 'inventory'):
                check_go([], root)

    def test_multiple_go_documents_and_malformed_tail(self):
        self.assertEqual(list(documents(' {"a":1}\n {"b":2}\n')), [{'a': 1}, {'b': 2}])
        with self.assertRaises(ValueError):
            list(documents('{} invalid'))


if __name__ == '__main__':
    unittest.main()
