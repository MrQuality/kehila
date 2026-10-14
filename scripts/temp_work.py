"""Owned temporary build directories with bounded Windows lock cleanup."""
from contextlib import contextmanager
from pathlib import Path
import re
import shutil
import sys
import tempfile
import time


def _remove_owned(work, root, prefix):
    if work.resolve() != work or work.parent != root or not work.name.startswith(prefix):
        raise ValueError('Refusing cleanup outside the allocated temporary directory')
    deadline = time.monotonic() + 30
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
def owned_directory(prefix):
    if re.fullmatch(r'[A-Za-z0-9_-]+', prefix) is None:
        raise ValueError('Temporary directory prefix must be a simple name')
    root = Path(tempfile.gettempdir()).resolve()
    work = Path(tempfile.mkdtemp(prefix=prefix, dir=root)).resolve()
    failure = None
    try:
        yield work
    except BaseException as error:
        failure = error
        raise
    finally:
        try:
            _remove_owned(work, root, prefix)
        except OSError as cleanup_error:
            if failure is not None:
                raise failure from cleanup_error
            raise
