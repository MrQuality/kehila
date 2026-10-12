#!/usr/bin/env python3
"""Verify the native foundation in an explicitly supplied disposable Podman container.

The caller creates and removes the container. This command never resets an
existing schema or drops cluster roles. It requires a fresh isolated database.
"""
import argparse
import ctypes
import json
import platform
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
IMAGE = 'docker.io/library/postgres@sha256:0ea6700a3b4f0ae6ce746519073558aed4d88a79d8d07622a9a644946c7319c4'
DESTINATION = '/opt/kehila-foundation-check'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def preflight():
    system = platform.system()
    if system == 'Windows':
        class MemoryStatus(ctypes.Structure):
            _fields_ = [('length', ctypes.c_ulong), ('load', ctypes.c_ulong)] + [
                (name, ctypes.c_ulonglong) for name in
                ('total', 'available', 'page_total', 'page_available',
                 'virtual_total', 'virtual_available', 'extended')]
        status = MemoryStatus()
        status.length = ctypes.sizeof(status)
        require(ctypes.windll.kernel32.GlobalMemoryStatusEx(ctypes.byref(status)),
                'cannot read available host memory')
        available = status.available
    elif system == 'Linux':
        memory = dict(line.split(':', 1) for line in Path('/proc/meminfo').read_text().splitlines())
        available = int(memory['MemAvailable'].split()[0]) * 1024
    else:
        raise ValueError('native foundation checks support Windows and Linux only')
    require(available >= 3 * 1024**3, 'at least 3 GiB available host memory required')
    require(shutil.disk_usage(ROOT).free >= 2 * 1024**3, 'at least 2 GiB free disk required')


def validate_container(info):
    config, host = info['Config'], info['HostConfig']
    require(info['State']['Running'], 'container must already be running')
    require((config.get('Labels') or {}).get('purpose') == 'kehila-postgresql-test',
            'container must have purpose=kehila-postgresql-test label')
    require(config['Image'] == IMAGE, 'use the documented pinned PostgreSQL image')
    require(host['NetworkMode'] == 'none' and not host.get('PortBindings'),
            'container must have no networking or published ports')
    require(not host.get('Privileged') and not host.get('Binds') and not info.get('Mounts'),
            'container must be unprivileged with no persistent or host mounts')
    require(0 < host['Memory'] <= 256 * 1024**2 and 0 < host['NanoCpus'] <= 10**9
            and 0 < host['PidsLimit'] <= 128, 'bounded memory, CPU and process limits required')
    tmpfs = host.get('Tmpfs') or {}
    require(set(tmpfs) == {'/var/lib/postgresql/data'}
            and 'size=256m' in tmpfs['/var/lib/postgresql/data'].split(','),
            'database must use the documented 256 MiB ephemeral tmpfs')


class DatabaseCheck:
    def __init__(self, podman, container):
        self.podman, self.container = podman, container

    def command(self, *args, input=None):
        return subprocess.run(self.podman + list(args), input=input, capture_output=True,
                              text=True, encoding='utf-8', timeout=60)

    def sql(self, statement=None, file=None, expected_state=None, expected_message=None,
            user='postgres'):
        require(expected_message is None or expected_state is not None,
                'expected message requires a SQLSTATE')
        args = ['exec', '-i', self.container, 'env', 'LC_ALL=C',
                'psql', '-X', '-U', user, '-d', 'postgres',
                '-v', 'ON_ERROR_STOP=1', '-v', 'VERBOSITY=verbose']
        if file:
            args += ['-f', file]
        completed = self.command(*args, input=statement)
        if expected_state is not None:
            # ON_ERROR_STOP uses exit 3 for script SQL errors; execution and
            # connection failures must never satisfy a negative database test.
            error = re.search(r'^(?:psql:[^\r\n]+:\d+:\s*)?ERROR:[ \t]+([0-9A-Z]{5}):[ \t]*(.*)$',
                              completed.stderr, re.MULTILINE)
            require(completed.returncode == 3 and error is not None
                    and error.group(1) == expected_state
                    and (expected_message is None or expected_message in error.group(2)),
                    'expected database rejection absent: ' + expected_state + '\n' + completed.stderr)
        else:
            require(completed.returncode == 0, completed.stderr)
        return completed.stdout

    def run(self):
        info = self.command('inspect', self.container)
        require(info.returncode == 0, info.stderr)
        validate_container(json.loads(info.stdout)[0])
        self.sql("DO $$ BEGIN IF EXISTS (SELECT FROM pg_namespace WHERE nspname='kehila') "
                 "OR EXISTS (SELECT FROM pg_roles WHERE rolname IN "
                 "('kehila_owner','kehila_creator','foundation_serving','foundation_untrusted')) "
                 "THEN RAISE EXCEPTION 'fresh database required'; END IF; END $$;")
        copied = self.command('cp', str(ROOT / 'storage/postgresql'),
                              self.container + ':' + DESTINATION)
        require(copied.returncode == 0, copied.stderr)
        installer = (ROOT / 'storage/postgresql/install.sql').read_text(encoding='utf-8')
        # Fail after all DDL, before COMMIT, to prove roles/schema roll back together.
        failed = installer.replace('COMMIT;', 'SELECT 1/0;\nCOMMIT;')
        self.sql("\\cd " + DESTINATION + "\n" + failed, expected_state='22012')
        self.sql("DO $$ BEGIN IF EXISTS (SELECT FROM pg_namespace WHERE nspname='kehila') "
                 "OR EXISTS (SELECT FROM pg_roles WHERE rolname IN ('kehila_owner','kehila_creator')) "
                 "THEN RAISE EXCEPTION 'installation rollback left objects'; END IF; END $$;")
        self.sql(file=DESTINATION + '/install.sql')
        self.sql(file=DESTINATION + '/install.sql', expected_state='P0001',
                 expected_message='fresh dedicated database')
        tests = ROOT / 'tests/integration'
        self.sql((tests / 'postgresql_foundation.sql').read_text(encoding='utf-8') + '\n' +
                 (tests / 'postgresql_creation.sql').read_text(encoding='utf-8'))
        # Use a genuinely separate serving connection, with no administrator session state.
        self.sql("BEGIN; SELECT * FROM kehila.lock_actor('actor'); "
                 "SELECT * FROM kehila.lock_operation((SELECT operation_row_id FROM "
                 "kehila.operations WHERE target_project_id='created')); ROLLBACK;",
                 user='foundation_serving')
        self.sql("SET ROLE kehila_owner;", expected_state='42501', user='foundation_serving')
        mutation = (tests / 'postgresql_payload_mutation.sql').read_text(encoding='utf-8')
        self.sql('BEGIN;\n' + mutation + '\nROLLBACK;')
        for trigger, label in (('operation_payload_immutable', 'request bytes'),
                               ('operation_payload_retirement_required', 'payload replacement')):
            self.sql('BEGIN; ALTER TABLE kehila.operation_payloads DISABLE TRIGGER ' +
                     trigger + ';\n' + mutation + '\nROLLBACK;',
                     expected_state='P0001', expected_message='payload mutation accepted: ' + label)
        self.sql('BEGIN;\n' + mutation + '\nROLLBACK;')
        regression = (tests / 'postgresql_payload_identity.sql').read_text(encoding='utf-8')
        self.sql('BEGIN;\n' + regression + '\nROLLBACK;')
        self.sql('BEGIN; ALTER TABLE kehila.operation_payloads DISABLE TRIGGER '
                 'operation_payload_immutable;\n' + regression + '\nROLLBACK;',
                 expected_state='P0001',
                 expected_message='identity regression did not reject reassignment')
        # Session termination rolls back the disabled trigger. Prove restoration explicitly.
        self.sql('BEGIN;\n' + regression + '\nROLLBACK;')
        print('PASS: atomic installation/rollback, reinstall rejection, scalar/core/creation '
              'integrity, role separation and payload mutation detection (PostgreSQL 16)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--disposable-container', required=True)
    parser.add_argument('--podman-connection')
    args = parser.parse_args()
    for name in (args.disposable_container, args.podman_connection):
        require(name is None or re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', name),
                'invalid container or connection name')
    preflight()
    podman = ['podman']
    if args.podman_connection:
        podman += ['--connection', args.podman_connection]
    DatabaseCheck(podman, args.disposable_container).run()


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print('ERR_POSTGRESQL_CHECK: ' + str(error), file=sys.stderr)
        sys.exit(3)
