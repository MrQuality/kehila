"""A work deadline owned by the verifier, with separate bounded cleanup time."""
from contextlib import contextmanager
from contextvars import ContextVar
import math
import os
import time

_inherited = object()
_deadline = ContextVar('verification_deadline', default=_inherited)


class DeadlineExceeded(TimeoutError):
    pass


def validate_seconds(seconds, *, maximum=600):
    if (type(seconds) not in (int, float) or type(maximum) not in (int, float)
            or not math.isfinite(maximum) or maximum <= 0
            or not math.isfinite(seconds) or not 0 < seconds <= maximum):
        raise ValueError(f'Verification work timeout must be positive and at most {maximum} seconds')


@contextmanager
def work_budget(seconds, *, maximum=600):
    validate_seconds(seconds, maximum=maximum)
    token = _deadline.set(time.monotonic() + seconds)
    try:
        yield
    finally:
        _deadline.reset(token)


def command_timeout(limit):
    deadline = _deadline.get()
    if deadline is _inherited:
        value = os.environ.get('KEHILA_VERIFICATION_DEADLINE')
        deadline = float(value) if value is not None else None
        if deadline is not None and not math.isfinite(deadline):
            raise ValueError('Invalid inherited verification deadline')
    if deadline is None:
        return limit
    remaining = deadline - time.monotonic()
    if remaining <= 0:
        raise DeadlineExceeded('Verification work deadline exceeded')
    return min(limit, remaining)


@contextmanager
def cleanup_time():
    token = _deadline.set(None)
    try:
        yield
    finally:
        _deadline.reset(token)


def owner_environment(environment, limit):
    """Pass a monotonic deadline to a cooperative child on this same host."""
    return dict(environment, KEHILA_VERIFICATION_DEADLINE=str(time.monotonic() + command_timeout(limit)))
