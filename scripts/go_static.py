#!/usr/bin/env python3
"""Check Go formatting and vet the same modules as the shared test runner."""
import subprocess
import sys

from go_test import GO_PACKAGES, ROOT


def main():
    files = sorted(str(p.relative_to(ROOT)) for p in (ROOT / 'go').rglob('*.go'))
    if not files:
        raise RuntimeError('No Go files found; formatting check cannot be empty')
    result = subprocess.run(['gofmt', '-l', *files], cwd=ROOT, check=True,
                            capture_output=True, text=True, timeout=60)
    if result.stdout.strip():
        raise RuntimeError('Run gofmt on:\n' + result.stdout.strip())
    subprocess.run(['go', 'vet', *GO_PACKAGES], cwd=ROOT, check=True, timeout=300)


if __name__ == '__main__':
    try:
        main()
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        print(f'ERR_GO_STATIC: {error}', file=sys.stderr)
        sys.exit(1)
