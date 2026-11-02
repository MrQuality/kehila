"""Establish a container-local work deadline from a remaining duration."""
import math
import os
import subprocess
import sys
import time


def main():
    seconds = float(sys.argv[1])
    command = sys.argv[2:]
    if not math.isfinite(seconds) or seconds <= 0 or not command:
        raise ValueError('Expected a positive remaining duration and command')
    environment = dict(os.environ, KEHILA_VERIFICATION_DEADLINE=str(time.monotonic() + seconds))
    try:
        return subprocess.run(command, env=environment, timeout=seconds).returncode
    except subprocess.TimeoutExpired:
        # The host coordinator removes the entire owned runner on failure,
        # including descendants of an expired command.
        print('CONTAINER_WORK_DEADLINE_EXCEEDED', file=sys.stderr)
        return 124


if __name__ == '__main__':
    sys.exit(main())
