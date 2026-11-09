"""Closed native-fixture resource profiles; selecting one does not grant execution."""
from dataclasses import dataclass
from storage_growth_plan import select


@dataclass(frozen=True)
class DatabaseLimits:
    # tmpfs consumes cgroup memory. Keep both hard limits coupled and reserve
    # 64 MiB for PostgreSQL/process/query overhead before each storage write.
    mib: int = 256

    def __post_init__(self):
        if type(self.mib) is not int or self.mib not in (256, 512):
            raise ValueError('Unknown native database resource profile')

    @property
    def memory_bytes(self):
        return self.mib * 1024**2

    @property
    def stop_bytes(self):
        return (self.mib - 64) * 1024**2


DEFAULT_LIMITS = DatabaseLimits()


def storage_limits(index, profile='default'):
    sample = select([index])[0]
    if profile not in ('default', 'extended') or (profile == 'extended' and sample.index < 20):
        raise ValueError('Extended storage resource profile requires a near-boundary SHA256 sample')
    return DatabaseLimits(512 if profile == 'extended' else 256)
