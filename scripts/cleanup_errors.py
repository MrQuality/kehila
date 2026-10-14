"""Retain teardown failures without rewriting the primary exception's cause."""
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
