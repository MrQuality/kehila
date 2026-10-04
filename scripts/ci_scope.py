"""Select only known prose changes for lightweight CI; uncertainty runs full CI."""
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location('commit_policy', ROOT / '.githooks/pre_commit.py')
POLICY = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(POLICY)


def choose_scope(event, root):
    if not isinstance(event, dict):
        return 'full'
    pr = event.get('pull_request')
    pr_base = pr.get('base') if isinstance(pr, dict) else None
    base = pr_base.get('sha') if isinstance(pr_base, dict) else None
    base = base or event.get('before')
    if (not isinstance(base, str) or not re.fullmatch(r'[0-9a-fA-F]{40}|[0-9a-fA-F]{64}', base)
            or set(base) == {'0'}):
        return 'full'
    try:
        changed = subprocess.check_output(
            ['git', 'diff', '--name-only', '--no-renames', '-z', base, 'HEAD', '--'],
            cwd=root, stderr=subprocess.PIPE, timeout=30).decode('utf-8').split('\0')
    except (OSError, UnicodeError, subprocess.SubprocessError):
        return 'full'
    paths = [path for path in changed if path]
    return 'docs' if POLICY.verification_scope(paths) == 'docs' else 'full'


def main():
    try:
        event = json.loads(Path(os.environ['GITHUB_EVENT_PATH']).read_text(encoding='utf-8'))
    except (KeyError, OSError, ValueError):
        event = None
    scope = choose_scope(event, ROOT)
    with Path(os.environ['GITHUB_OUTPUT']).open('a', encoding='utf-8') as output:
        output.write(f'scope={scope}\n')
    print(f'CI verification scope: {scope}')


if __name__ == '__main__':
    main()
