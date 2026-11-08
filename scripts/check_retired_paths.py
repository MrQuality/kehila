#!/usr/bin/env python3
"""Reject retired prototype bindings in active inputs, preserving historical records."""
from pathlib import Path
import re
import sys

from go_inventory import GO_MODULES
from healthcheck import PORTS
from test_services import SERVICES
from verification_ownership import GROUPS

ROOT = Path(__file__).resolve().parents[1]
# Executed/imported/compiled inputs of the shared matrix's retained oracle checks.
# Standalone experiment entry points and historical observations are separate.
ORACLE_INPUTS = (
    'experiments/SP-002/check_seed_oracle.py', 'experiments/SP-002/manage.py',
    'experiments/SP-002/seed_oracle.rs', 'experiments/SP-003/check.py',
    'experiments/SP-003/build.py', 'experiments/SP-003/run.py',
    'experiments/SP-003/containers.py', 'experiments/SP-003/main.rs',
    'experiments/SP-003/codec.rs', 'experiments/SP-003/tests.rs',
)
REQUIRED = ('Cargo.toml', 'Cargo.lock', 'go.work', 'docker-compose.yml', *ORACLE_INPUTS)
RETIRED_PATHS = ('src/io/task_worker', 'go/io/task_api',
                 'tests/integration/task_path.py', 'tests/integration/live_task_worker.py',
                 'tests/integration/live_task_api.py', 'tests/integration/live_go_rust.py')
RETIRED = re.compile(
    r'\b(?:ferretdb|mongodb|bson|task_worker|task_api|task-api)\b'
    r'|\b(?:task_path|live_task_worker|live_task_api|live_go_rust)\.py\b'
    r'|\b(?:KEHILA_)?MONGO(?:DB)?_[A-Z_]+\b'
    r'|\bKEHILA_(?:WORKER_URL|WORKER_LISTEN_ADDR|TASK_DB|TASK_COLLECTION|API_URL)\b',
    re.IGNORECASE)
DIRECTORIES = ('src', 'go', 'scripts', 'packages', '.github', 'config', 'tools', 'tests')
SUFFIXES = {'.py', '.rs', '.go', '.toml', '.lock', '.mod', '.sum', '.json',
            '.yml', '.yaml', '.js', '.cjs', '.ts', '.sql'}


def active_paths(root):
    paths = {root / name for name in REQUIRED}
    paths.update(root / name for name in ('.env.example', 'package.json', 'go.work.sum',
                                        'pnpm-workspace.yaml', 'tools/verification/Containerfile',
                                        '.githooks/pre_commit.py', '.githooks/pre-commit',
                                        '.githooks/pre-commit.bat'))
    for directory in DIRECTORIES:
        paths.update(path for path in (root / directory).rglob('*')
                     if path.suffix in SUFFIXES
                     and not {'__pycache__', 'node_modules', 'target', '.git'}.intersection(
                         path.relative_to(root).parts))
    # Policies and the single non-executable rejection-data file deliberately
    # contain retired spellings. No executable test module is exempted.
    paths.difference_update(root / name for name in (
        'scripts/check_retired_paths.py', 'scripts/check_branding.py',
        'tests/fixtures/retired-bindings.json'))
    return sorted(path for path in paths if path.is_file())


def violations(root):
    root = Path(root)
    errors = [name + ': required active input is missing' for name in REQUIRED
              if not (root / name).is_file()]
    errors.extend(name + ': retired entry point remains' for name in RETIRED_PATHS
                  if (root / name).exists())
    catalogs = {'GO_MODULES': GO_MODULES, 'SERVICES': SERVICES, 'PORTS': PORTS,
                'GROUPS': (service for group in GROUPS for service in group)}
    for name, entries in catalogs.items():
        if any(RETIRED.search(entry) for entry in entries):
            errors.append(name + ': retired binding in active inventory')
    for path in active_paths(root):
        name = path.relative_to(root).as_posix()
        try:
            text = path.read_text(encoding='utf-8')
        except UnicodeDecodeError:
            errors.append(f'{name}: active input is not UTF-8')
            continue
        for number, line in enumerate(text.splitlines(), 1):
            if RETIRED.search(line):
                errors.append(f'{name}:{number}: retired active binding')
    return errors


def main():
    errors = violations(ROOT)
    if errors:
        print('\n'.join(errors), file=sys.stderr)
        return 1
    print('Retired prototype absent from active inputs')
    return 0


if __name__ == '__main__':
    sys.exit(main())
