#!/usr/bin/env python3
"""Research-only Linux system sampler; raw child output stays outside source."""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import threading
import time

PHASES = ('image', 'database-image', 'dependencies', 'native')


def memory_bytes(text):
    fields = {}
    for line in text.splitlines():
        name, _, value = line.partition(':')
        if name in {'MemTotal', 'MemAvailable'}:
            parts = value.split()
            if len(parts) != 2 or parts[1] != 'kB':
                raise ValueError('Unexpected Linux memory units')
            fields[name] = int(parts[0]) * 1024
    total, available = fields['MemTotal'], fields['MemAvailable']
    if not 0 <= available <= total or total <= 0:
        raise ValueError('Invalid Linux memory observation')
    return total, available


class Samples:
    def __init__(self, storage):
        self.storage = storage
        self.count = 0
        self.total = None
        self.minimum_available = None
        self.minimum_disk = None
        self.error = False
        self.stop = threading.Event()

    def sample(self):
        total, available = memory_bytes(Path('/proc/meminfo').read_text())
        disk = shutil.disk_usage(self.storage).free
        if self.total is not None and total != self.total:
            raise ValueError('System memory total changed during sampling')
        self.total = total
        self.minimum_available = available if self.count == 0 else min(self.minimum_available, available)
        self.minimum_disk = disk if self.count == 0 else min(self.minimum_disk, disk)
        self.count += 1

    def collect(self):
        while not self.stop.wait(1):
            try:
                self.sample()
            except (OSError, ValueError, KeyError):
                self.error = True
                return


def qualified_exit(child_exit, sampling_error):
    # Sampling failure must not conceal the original workload failure.
    return child_exit if child_exit != 0 else (1 if sampling_error else 0)


def measure(phase, command, output, storage):
    output.mkdir(mode=0o700, parents=False, exist_ok=False)
    before = shutil.disk_usage(storage).free
    samples = Samples(storage)
    samples.sample()  # No workload starts when initial observation fails.
    started = time.monotonic()
    collector = threading.Thread(target=samples.collect, daemon=True)
    collector.start()
    child_exit = 1
    try:
        with (output / 'child.log').open('xb') as log:
            child_exit = subprocess.run(command, stdout=log, stderr=log, check=False).returncode
    finally:
        samples.stop.set()
        collector.join(timeout=5)
        samples.error |= collector.is_alive()
        if not collector.is_alive():
            try:
                samples.sample()
            except (OSError, ValueError, KeyError):
                samples.error = True
    report = {
        'phase': phase,
        'child_exit': child_exit,
        'sampling_error': samples.error,
        'seconds': round(time.monotonic() - started, 3),
        'sample_count': samples.count,
        'sample_interval_seconds': 1,
        'system_memory_total_bytes': samples.total,
        'system_memory_available_min_bytes': samples.minimum_available,
        'storage_free_before_bytes': before,
        'storage_free_min_bytes': samples.minimum_disk,
        'storage_free_after_bytes': shutil.disk_usage(storage).free,
    }
    (output / 'measurement.json').write_text(json.dumps(report, sort_keys=True) + '\n')
    summary = os.environ.get('GITHUB_STEP_SUMMARY')
    if summary:
        with open(summary, 'a', encoding='utf-8') as stream:
            stream.write('```json\n' + json.dumps(report, sort_keys=True) + '\n```\n')
    print(json.dumps(report, sort_keys=True))
    return qualified_exit(child_exit, samples.error)


def parse_arguments(argv):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('phase', choices=PHASES)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--storage', type=Path, required=True)
    if '--' not in argv:
        parser.error('Separate the explicit workload command with --')
    boundary = argv.index('--')
    args = parser.parse_args(argv[:boundary])
    source = Path(__file__).resolve().parents[2]
    output = args.output.resolve()
    if not args.output.is_absolute() or output == source or source in output.parents:
        parser.error('Measurements and raw logs require a fresh external output directory')
    command = argv[boundary + 1:]
    if not command:
        parser.error('An explicit workload command is required')
    return args, command, output


def main():
    args, command, output = parse_arguments(sys.argv[1:])
    return measure(args.phase, command, output, args.storage.resolve())


if __name__ == '__main__':
    raise SystemExit(main())
