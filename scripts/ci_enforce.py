#!/usr/bin/env python3
"""Exercise the immutable commit gate with the same event policy as CI."""
from pathlib import Path
import os
import subprocess
import sys
import tempfile

from ci_scope import choose_scope, event_context, read_event

ROOT = Path(__file__).resolve().parents[1]

def main():
    event = read_event()
    context = event_context(event, os.environ.get('GITHUB_EVENT_NAME'))
    scope = choose_scope(event, ROOT)
    qualification = ['--qualification'] if scope == 'full' else []
    print('CI enforced scope:', scope, flush=True)
    if context.base is not None:
        subprocess.run([sys.executable, '.githooks/pre_commit.py', '--base', context.base,
                        *qualification], cwd=ROOT, check=True)
    else:
        # Root push: preserve staged source inside a disposable unborn branch.
        with tempfile.TemporaryDirectory(prefix='kehila-ci-root-') as temporary:
            subprocess.run(['git', 'clone', '--no-hardlinks', str(ROOT), temporary], check=True)
            branch = 'refs/heads/kehila-ci-root'
            subprocess.run(['git', 'update-ref', '-d', branch], cwd=temporary, check=True)
            subprocess.run(['git', 'symbolic-ref', 'HEAD', branch], cwd=temporary, check=True)
            subprocess.run([sys.executable, '.githooks/pre_commit.py', *qualification],
                           cwd=temporary, check=True)


if __name__ == '__main__':
    try:
        main()
    except ValueError as error:
        raise SystemExit('CI_SELECTION_FAILED: ' + str(error)) from None
