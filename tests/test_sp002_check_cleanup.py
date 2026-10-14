"""The frozen oracle's check driver owns and removes its generated executables."""
from contextlib import contextmanager
import importlib.util
import json
from pathlib import Path
import sys
import unittest
from unittest.mock import Mock, patch

root = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(root / 'experiments/SP-002'))
spec = importlib.util.spec_from_file_location('seed_check_cleanup', root / 'experiments/SP-002/check_seed_oracle.py')
driver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(driver)
sys.path.insert(0, str(root / 'scripts'))
from temp_work import owned_directory


class CheckCleanupTests(unittest.TestCase):
    def exercise(self, failure):
        allocated = []

        @contextmanager
        def capture(prefix):
            with owned_directory(prefix) as folder:
                allocated.append(folder)
                yield folder

        def command(args):
            if args[0] == 'rustc' and failure:
                raise OSError('compiler failed')
            if args[-1] in ('hours', 'points'):
                return json.dumps({'project': {'name': args[3], 'estimate_unit': args[-1]}}).encode()
            return b''

        with patch.object(driver, 'owned_directory', capture), \
                patch.object(driver, 'cargo_library', return_value=Path('unused/libtask_contract.rlib')), \
                patch.object(driver, 'run', side_effect=command), \
                patch.object(driver.subprocess, 'run', return_value=Mock(returncode=2, stdout=b'', stderr=b'bad args')):
            if failure:
                with self.assertRaisesRegex(OSError, 'compiler failed'):
                    driver.main()
            else:
                driver.main()
        self.assertEqual(len(allocated), 1)
        self.assertFalse(allocated[0].exists())

    def test_check_success_removes_outputs(self):
        self.exercise(False)

    def test_compile_failure_removes_outputs(self):
        self.exercise(True)
