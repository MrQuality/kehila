"""Owned temporary build directories with bounded Windows lock cleanup."""
from cleanup_errors import retain_cleanup_failure
from contextlib import contextmanager
from pathlib import Path
import re
import math
import shutil
import sys
import tempfile
import time


def _remove_owned(work, root, prefix, *, retry_seconds=30):
    if work.resolve() != work or work.parent != root or not work.name.startswith(prefix):
        raise ValueError('Refusing cleanup outside the allocated temporary directory')
    deadline = time.monotonic() + retry_seconds
    while work.exists():
        try:
            shutil.rmtree(work)
            return
        except PermissionError as error:
            if (sys.platform != 'win32' or getattr(error, 'winerror', None) not in {5, 32, 33}
                    or time.monotonic() >= deadline):
                raise
            time.sleep(0.25)


@contextmanager
def owned_directory(prefix, *, parent=None, retry_seconds=30):
    if re.fullmatch(r'[A-Za-z0-9_-]+', prefix) is None:
        raise ValueError('Temporary directory prefix must be a simple name')
    if not math.isfinite(retry_seconds) or not 0 <= retry_seconds <= 30:
        raise ValueError('Cleanup retry budget must be between zero and 30 seconds')
    root = Path(tempfile.gettempdir() if parent is None else parent).resolve()
    work = Path(tempfile.mkdtemp(prefix=prefix, dir=root)).resolve()
    failure = None
    try:
        yield work
    except BaseException as error:
        failure = error
        raise
    finally:
        try:
            _remove_owned(work, root, prefix, retry_seconds=retry_seconds)
        except OSError as cleanup_error:
            if failure is not None:
                retain_cleanup_failure(failure, cleanup_error, 'temporary directory')
                raise failure
            raise
