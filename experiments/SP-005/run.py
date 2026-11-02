"""Opt-in structural mapping checks. Caller owns the disposable container.

Only the structural baseline is used, without compilation. Service/resource
approval remains separate; import runs no work or service discovery.
"""
import argparse
import hashlib
import json
import math
from pathlib import Path
import platform
import re
import sys
import time

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'scripts'))
import check_postgresql as native
from cleanup_errors import run_main


class MappingCheck(native.DatabaseCheck):
    """One finite budget across inspection, installation and mapping cases."""

    def __init__(self, podman, container, seconds=180):
        native.require(math.isfinite(seconds) and 0 < seconds <= 600,
                       'work budget must be finite and in (0, 600] seconds')
        super().__init__(podman, container)
        self.deadline = time.monotonic() + seconds

    def command(self, *args, input=None):
        remaining = self.deadline - time.monotonic()
        native.require(remaining > 0, 'SP-005 work budget expired')
        return super().command(*args, input=input, timeout=min(60, remaining))

    def sql(self, statement=None, **kwargs):
        if statement is not None:
            statement = ("SET statement_timeout='10s'; SET lock_timeout='2s'; "
                         "SET idle_in_transaction_session_timeout='10s';\n" + statement)
        return super().sql(statement, **kwargs)


def require_marker(output, marker):
    native.require(isinstance(output, str) and output.splitlines().count(marker) == 1,
                   'native oracle marker missing or duplicated: ' + marker)


def run(database):
    # Owns fresh-container checks and structural baseline; never skip on a marker
    # supplied by a caller or assume an existing database is disposable.
    database.prepare_structure()
    database.sql((SOURCE / 'overlay.sql').read_text(encoding='utf-8'))
    require_marker(database.sql('\\pset tuples_only on\n\\pset format unaligned\n' +
                   (SOURCE / 'fixtures.sql').read_text(encoding='utf-8')), 'SP005_FIXTURE_PASS')
    for name, user, marker in (
            ('serving.sql', 'foundation_serving', 'SP005_SERVING_PASS'),
            ('owner.sql', 'postgres', 'SP005_OWNER_PASS')):
        statement = ('\\pset tuples_only on\n\\pset format unaligned\n' +
                     (SOURCE / name).read_text(encoding='utf-8'))
        require_marker(database.sql(statement, user=user), marker)
    database.sql('SELECT * FROM sp005.creation_intents;', user='foundation_untrusted',
                 expected_state='42501', expected_message='permission denied')
    sources = [*sorted(SOURCE.glob('*.sql')), SOURCE / 'run.py',
               ROOT / 'scripts/check_postgresql.py',
               ROOT / 'tests/integration/postgresql_foundation.sql',
               ROOT / 'tests/integration/postgresql_creation.sql',
               *sorted((ROOT / 'storage/postgresql').glob('*.sql'))]
    return {
        'scope': 'SP-005 physical mapping structural subset',
        'outcome': 'passed',
        'platform': platform.system(),
        'sources': {path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in sources},
        'not_qualified': [
            'authenticated installation/actor resolution and public transport',
            'atomic application project creation with mapping',
            'concurrent retries/collision re-entry and uncertain commit',
            'typed historical replay, expiry/retirement and supported restore',
            'aggregate resources and production schema/dependency adoption',
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--disposable-container', required=True)
    parser.add_argument('--podman-connection')
    parser.add_argument('--timeout', type=float, default=180,
                        help='Total subprocess-work budget, seconds (0, 600]')
    args = parser.parse_args()
    for value in (args.disposable_container, args.podman_connection):
        native.require(value is None or re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', value),
                       'invalid container or connection name')
    native.require(math.isfinite(args.timeout) and 0 < args.timeout <= 600,
                   'work budget must be finite and in (0, 600] seconds')
    native.preflight()
    podman = ['podman']
    if args.podman_connection:
        podman += ['--connection', args.podman_connection]
    result = run(MappingCheck(podman, args.disposable_container, args.timeout))
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_SP005_CHECK'))
