"""Owned build-directory cleanup must retry only transient Windows locks."""
from pathlib import Path
import shutil
import sys
import tempfile
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'scripts'))
import temp_work


def locked(code=5):
    error = PermissionError('locked executable')
    error.winerror = code
    return error


class OwnedDirectoryTests(unittest.TestCase):
    def test_owned_directory_is_removed_after_use(self):
        with temp_work.owned_directory('kehila-cleanup-test-') as work:
            (work / 'artifact').write_bytes(b'build output')
        self.assertFalse(work.exists())

    def test_invalid_prefix_does_not_allocate(self):
        with patch.object(temp_work.tempfile, 'mkdtemp') as allocate:
            for prefix in ('', '../outside', 'name/child', 'name\\child'):
                with self.subTest(prefix=prefix), self.assertRaises(ValueError):
                    with temp_work.owned_directory(prefix):
                        self.fail('invalid prefix admitted')
            allocate.assert_not_called()

    def test_windows_transient_locks_retry_then_remove(self):
        remove = shutil.rmtree
        for code in (5, 32, 33):
            with self.subTest(code=code), patch.object(temp_work.sys, 'platform', 'win32'), \
                    patch.object(temp_work.time, 'sleep') as sleep, \
                    patch.object(temp_work.shutil, 'rmtree', side_effect=[locked(code), None]) as retry:
                with temp_work.owned_directory('kehila-cleanup-test-') as work:
                    pass
                self.assertEqual(retry.call_count, 2)
                sleep.assert_called_once_with(0.25)
            remove(work)

    def test_other_platform_or_error_does_not_retry(self):
        for platform, code in (('linux', 5), ('win32', 87)):
            with self.subTest(platform=platform, code=code), \
                    patch.object(temp_work.sys, 'platform', platform), \
                    patch.object(temp_work.time, 'sleep') as sleep, \
                    patch.object(temp_work.shutil, 'rmtree', side_effect=locked(code)):
                with self.assertRaises(PermissionError):
                    with temp_work.owned_directory('kehila-cleanup-test-') as work:
                        pass
                sleep.assert_not_called()
            shutil.rmtree(work)

    def test_deadline_keeps_cleanup_failure_visible(self):
        with patch.object(temp_work.sys, 'platform', 'win32'), \
                patch.object(temp_work.time, 'monotonic', side_effect=[10, 40]), \
                patch.object(temp_work.time, 'sleep') as sleep, \
                patch.object(temp_work.shutil, 'rmtree', side_effect=locked()):
            with self.assertRaises(PermissionError):
                with temp_work.owned_directory('kehila-cleanup-test-') as work:
                    pass
            sleep.assert_not_called()
        shutil.rmtree(work)

    def test_body_failure_is_preserved_with_cleanup_failure_as_cause(self):
        primary = RuntimeError('integration failed')
        cleanup = OSError('cleanup failed')
        with patch.object(temp_work.shutil, 'rmtree', side_effect=cleanup):
            with self.assertRaises(RuntimeError) as caught:
                with temp_work.owned_directory('kehila-cleanup-test-') as work:
                    raise primary
        self.assertIs(caught.exception, primary)
        self.assertIs(caught.exception.__cause__, cleanup)
        shutil.rmtree(work)

    def test_outside_owned_root_is_never_removed(self):
        root = Path(tempfile.gettempdir()).resolve()
        with patch.object(temp_work.shutil, 'rmtree') as remove:
            for work in (root, root / 'wrong-prefix', root.parent / 'kehila-test'):
                with self.subTest(work=work), self.assertRaises(ValueError):
                    temp_work._remove_owned(work, root, 'kehila-')
            remove.assert_not_called()
