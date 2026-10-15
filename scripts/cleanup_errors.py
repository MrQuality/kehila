"""Retain teardown failures without rewriting the primary exception's cause."""
from contextlib import ExitStack
import sys


def retain_cleanup_failure(primary, cleanup, resource):
    failures = getattr(primary, '_cleanup_failures', ())
    primary._cleanup_failures = (*failures, (resource, cleanup))


def print_failure(prefix, error):
    seen = set()

    def emit(label, failure):
        if id(failure) in seen:
            return
        seen.add(id(failure))
        print(label + ': ' + (str(failure) or type(failure).__name__), file=sys.stderr)
        if failure.__cause__ is not None:
            emit('CAUSE', failure.__cause__)
        for resource, cleanup in getattr(failure, '_cleanup_failures', ()):
            emit('CLEANUP FAILED (' + resource + ')', cleanup)
        if failure.__cause__ is None and not failure.__suppress_context__ and failure.__context__ is not None:
            emit('CONTEXT', failure.__context__)

    emit(prefix, error)


def run_main(main, prefix):
    """Report retained failures at every resource owner's CLI boundary."""
    try:
        return main()
    except KeyboardInterrupt as error:
        print_failure(prefix, error)
        return 130
    except Exception as error:
        print_failure(prefix, error)
        return 3


class CleanupStack:
    """Own resource teardown; preserve body errors and never honor suppression."""

    def __init__(self):
        self._stack = ExitStack()
        self._failure = None
        self._exception = (None, None, None)

    def __enter__(self):
        return self

    def callback(self, function, *args, resource, **kwargs):
        def cleanup():
            try:
                function(*args, **kwargs)
            except BaseException as error:
                if self._failure is None:
                    self._failure = error
                else:
                    retain_cleanup_failure(self._failure, error, resource)
        self._stack.callback(cleanup)

    def enter_context(self, manager, *, resource):
        value = manager.__enter__()
        self.callback(lambda: manager.__exit__(*self._exception), resource=resource)
        return value

    def __exit__(self, kind, error, traceback):
        self._failure = error
        self._exception = (kind, error, traceback)
        self._stack.close()
        if error is None and self._failure is not None:
            raise self._failure
        return False
