#!/usr/bin/env python3
"""Opt-in A1 owner-storage proof in a fresh, owned, networkless PostgreSQL container."""
import argparse
import hashlib
import json
import platform
from pathlib import Path
import re
import subprocess
import sys
import time

sys.dont_write_bytecode = True
SOURCE = Path(__file__).resolve().parent
ROOT = SOURCE.parents[2]
sys.path.insert(0, str(ROOT / 'scripts'))
sys.path.insert(0, str(ROOT / 'experiments/SP-004'))
from check_postgresql import IMAGE, preflight, validate_container
from cleanup_errors import run_main
from enrollment_cases import require, run
from authority_cases import run as authority_cases
from test_services import managed_runtime, service_group
from verification_budget import command_timeout, work_budget


class Database:
    def __init__(self, runtime, container):
        self.podman = runtime.engine
        self.container = container
        self.environment = runtime.environment

    def sql(self, statement: str, *, user: str = 'postgres', expected_state: str | None = None) -> str:
        require(len(statement.encode('utf-8')) <= 262144, 'fixture SQL exceeds 256 KiB')
        command = [*self.podman, 'exec', '-i', self.container, 'env', 'LC_ALL=C',
                   'psql', '-XqAt', '-U', user, '-d', 'postgres',
                   '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose']
        result = subprocess.run(command, input="SET statement_timeout='10s'; SET lock_timeout='5s'; "
                                + statement, capture_output=True, text=True, encoding='utf-8',
                                env=self.environment, cwd=ROOT, timeout=command_timeout(20))
        require(len(result.stdout.encode('utf-8')) <= 65536, 'fixture SQL response exceeds 64 KiB')
        match = re.search(r'^ERROR:\s+([0-9A-Z]{5}):', result.stderr, re.MULTILINE)
        state = match[1] if match else None
        if expected_state is not None:
            require(result.returncode == 3 and state == expected_state,
                    'expected SQLSTATE ' + expected_state + ', observed ' + str(state))
        else:
            # Never echo a SQL statement, verifier or raw database error in public output.
            require(result.returncode == 0, 'fixture SQL failed, SQLSTATE ' + str(state))
        return result.stdout.strip()


def foundation():
    """Expand only the installer's fixed public includes; do not modify that installer."""
    folder = ROOT / 'storage/postgresql'
    source = (folder / 'install.sql').read_text(encoding='utf-8')
    source = source.replace('\\set ON_ERROR_STOP on', '')
    for name in ('operations.sql', 'project_create.sql', 'access.sql'):
        source = source.replace('\\ir ' + name, (folder / name).read_text(encoding='utf-8'))
    require('\\ir' not in source, 'unrecognized foundation include')
    return source


def ready(db):
    deadline = time.monotonic() + 40
    while time.monotonic() < deadline:
        result = subprocess.run([*db.podman, 'exec', db.container, 'pg_isready', '-U', 'postgres'],
                                capture_output=True, env=db.environment, cwd=ROOT, timeout=command_timeout(5))
        if result.returncode == 0:
            return
        time.sleep(0.2)
    raise ValueError('owned PostgreSQL did not become ready')


def validate_small_workload(info):
    """A distinct resource profile, never an override of the foundation validator."""
    config, host = info['Config'], info['HostConfig']
    require(info['State']['Running'] and config['Image'] == IMAGE, 'unexpected database image/state')
    require((config.get('Labels') or {}).get('purpose') == 'kehila-postgresql-test',
            'database purpose label missing')
    require(host['NetworkMode'] == 'none' and not host.get('PortBindings'),
            'database must have no network or published ports')
    require(not host.get('Privileged') and not host.get('Binds') and not info.get('Mounts'),
            'database must have no privilege, persistent mount or host mount')
    require(host['Memory'] == 0 and host['NanoCpus'] == 0,
            'small-workload profile must not claim CPU/memory enforcement')
    tmpfs = host.get('Tmpfs') or {}
    require(set(tmpfs) == {'/var/lib/postgresql/data'}
            and 'size=256m' in tmpfs['/var/lib/postgresql/data'].split(','),
            'database must use the 256 MiB ephemeral data filesystem')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--podman-machine', required=True)
    parser.add_argument('--exclusive-podman-machine', action='store_true')
    parser.add_argument('--profile', choices=('capped', 'small-workload'), default='capped')
    args = parser.parse_args()
    require(args.exclusive_podman_machine, 'proof requires an initially stopped, exclusively owned machine')
    preflight()
    with work_budget(600), managed_runtime('podman', args.podman_machine, exclusive=True) as runtime:
        preflight()
        compose = 'compose.yml' if args.profile == 'capped' else 'compose.small-workload.yml'
        with service_group(runtime, ['postgres'], compose_file=SOURCE / compose) as containers:
            db = Database(runtime, containers['postgres'])
            inspection = subprocess.run([*db.podman, 'inspect', db.container], capture_output=True,
                                        text=True, encoding='utf-8', env=db.environment,
                                        check=True, cwd=ROOT, timeout=command_timeout(10))
            validator = validate_container if args.profile == 'capped' else validate_small_workload
            validator(json.loads(inspection.stdout)[0])
            preflight()
            ready(db)
            version = db.sql("SELECT current_setting('server_version');")
            db.sql(foundation())
            overlay = SOURCE / 'install.sql'
            db.sql(overlay.read_text(encoding='utf-8'))
            authority = SOURCE / 'authority.sql'
            db.sql(authority.read_text(encoding='utf-8'))
            db.sql("CREATE ROLE auth001_test_admin LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE "
                   "NOREPLICATION NOBYPASSRLS; "
                   "CREATE ROLE auth001_test_runtime LOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE "
                   "NOREPLICATION NOBYPASSRLS;")
            db.sql('GRANT auth001_admin TO auth001_test_admin; '
                   'GRANT auth001_runtime TO auth001_test_runtime;')
            passed = run(db)
            preflight()
            passed += authority_cases(db)
            engine_version = subprocess.run([*db.podman, '--version'], capture_output=True,
                                            text=True, encoding='utf-8', env=db.environment,
                                            cwd=ROOT, check=True, timeout=command_timeout(10)).stdout.strip()
            files = ('install.sql', 'authority.sql', 'run.py', 'enrollment_cases.py', 'authority_cases.py', compose)
            hashes = {name: hashlib.sha256((SOURCE / name).read_bytes().replace(b'\r\n', b'\n')).hexdigest()
                      for name in files}
    # Report success only after both owned context managers complete cleanup.
    print(json.dumps({'cases': passed, 'passed': len(passed), 'scope': 'A1 enrollment/startup/takeover',
                      'profile': args.profile, 'cgroup_limits_qualified': False,
                      'postgresql': version, 'engine': engine_version,
                      'python': platform.python_version(), 'host': platform.system(),
                      'source_sha256_lf': hashes, 'owned_cleanup_confirmed': True}))
    return 0


if __name__ == '__main__':
    sys.exit(run_main(main, 'A1 STORAGE PROOF'))
