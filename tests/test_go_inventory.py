"""Go source scope must agree with workspace, static checks and test inventory."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import sys

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('go_inventory', ROOT / 'scripts/go_inventory.py')
INVENTORY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(INVENTORY)


class GoInventoryTests(unittest.TestCase):
    def layout(self, root):
        for module in INVENTORY.GO_MODULES:
            directory = root / module
            directory.mkdir(parents=True)
            (directory / 'go.mod').write_text('module fixture\n', encoding='utf-8')
            (directory / 'fixture.go').write_text('package fixture\n', encoding='utf-8')
        return {'Use': [{'DiskPath': './' + module} for module in INVENTORY.GO_MODULES]}

    def test_known_layout_covers_every_source_and_package_pattern(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = self.layout(root)
            self.assertEqual(INVENTORY.validate_layout(root, workspace),
                             sorted(module + '/fixture.go' for module in INVENTORY.GO_MODULES))
            self.assertEqual(INVENTORY.GO_PACKAGES,
                             ['./' + module + '/...' for module in INVENTORY.GO_MODULES])

    def test_source_outside_inventory_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            workspace = self.layout(root)
            (root / 'go/orphan.go').write_text('package orphan\n', encoding='utf-8')
            with self.assertRaisesRegex(RuntimeError, 'outside inventoried modules'):
                INVENTORY.validate_layout(root, workspace)

    def test_unlisted_and_missing_modules_fail(self):
        for missing in (False, True):
            with self.subTest(missing=missing), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                workspace = self.layout(root)
                if missing:
                    (root / INVENTORY.GO_MODULES[0] / 'go.mod').unlink()
                else:
                    (root / 'go/extra').mkdir()
                    (root / 'go/extra/go.mod').write_text('module extra\n', encoding='utf-8')
                with self.assertRaisesRegex(RuntimeError, 'module inventory differs'):
                    INVENTORY.validate_layout(root, workspace)

    def test_workspace_omission_external_path_and_duplicates_fail(self):
        for entries in ([], [{'DiskPath': '../outside'}],
                        [{'DiskPath': './' + INVENTORY.GO_MODULES[0]}] * 2):
            with self.subTest(entries=entries), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                self.layout(root)
                with self.assertRaisesRegex(RuntimeError, 'workspace'):
                    INVENTORY.validate_layout(root, {'Use': entries})

    def test_inventory_failure_prevents_test_execution_and_temp_allocation(self):
        sys.path.insert(0, str(ROOT / 'scripts'))
        try:
            spec = importlib.util.spec_from_file_location('go_runner', ROOT / 'scripts/go_test.py')
            runner = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(runner)
        finally:
            sys.path.pop(0)
        with patch.object(runner, 'go_files', side_effect=RuntimeError('module omitted')), \
                patch.object(runner.subprocess, 'run') as run, \
                patch.object(runner.tempfile, 'mkdtemp') as allocate:
            with self.assertRaisesRegex(RuntimeError, 'module omitted'):
                runner.main()
            run.assert_not_called()
            allocate.assert_not_called()
