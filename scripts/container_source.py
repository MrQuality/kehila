"""Check exact copied snapshot contents before compiling or running tests."""
import hashlib
import json
from pathlib import Path
import sys


def verify_limits(memory_bytes, pids, cpus):
    cgroup = Path('/sys/fs/cgroup')
    memory = (cgroup / 'memory.max').read_text().strip()
    processes = (cgroup / 'pids.max').read_text().strip()
    quota, period = (cgroup / 'cpu.max').read_text().split()
    if (memory != str(memory_bytes) or processes != str(pids)
            or quota == 'max' or int(period) <= 0 or int(quota) != cpus * int(period)):
        raise ValueError('Effective cgroup-v2 runner limits differ from requested policy')


def main():
    verify_limits(int(sys.argv[2]), int(sys.argv[3]), float(sys.argv[4]))
    root = Path.cwd()
    expected = json.loads(Path(sys.argv[1]).read_text(encoding='utf-8'))
    actual = {}
    for path in root.rglob('*'):
        if path.is_symlink():
            raise ValueError('Copied source contains a symlink')
        if path.is_file():
            actual[path.relative_to(root).as_posix()] = hashlib.sha256(path.read_bytes()).hexdigest()
    if actual != expected:
        raise ValueError('Copied snapshot differs from the exported index')
    print('CONTAINER SOURCE: exact copied snapshot verified', flush=True)


if __name__ == '__main__':
    main()
