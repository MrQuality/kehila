#!/usr/bin/env python3
"""Opt-in owner-only WorkItem structure checks in a supplied disposable Podman DB.

The caller owns container/provider lifecycle and resource supervision. This never
upgrades an existing database, compiles code, starts services or changes trust.
It exercises installed version-three owner contracts; it does not qualify the
authenticated production WorkItem runtime or interface.
"""
import argparse
import json
from pathlib import Path
import re
import sys

from check_postgresql import DatabaseCheck, preflight, require, validate_container
from cleanup_errors import run_main

ROOT = Path(__file__).resolve().parents[1]
DESTINATION = '/opt/kehila-work-item-check'
FRESH = """DO $$ BEGIN
  IF EXISTS (SELECT FROM pg_namespace WHERE nspname IN ('kehila','kehila_auth'))
     OR EXISTS (SELECT FROM pg_roles WHERE rolname IN
       ('kehila_owner','kehila_creator','kehila_auth_runtime','kehila_auth_admin',
        'kehila_auth_runtime_login')) THEN
    RAISE EXCEPTION 'fresh dedicated WorkItem fixture required';
  END IF;
END; $$;"""
INSTALLED = """DO $$ BEGIN
  IF (SELECT array_agg(version ORDER BY version) FROM kehila.schema_migrations)
     IS DISTINCT FROM ARRAY[1,2,3] THEN
    RAISE EXCEPTION 'exact version-three product fixture required';
  END IF;
END; $$;"""


def check_core_wait(database):
    completed = database.command('exec', database.container, 'timeout', '--kill-after=2s', '25s',
                                 'sh', '/opt/kehila-work-item-core-wait.sh', timeout=35)
    require(completed.returncode == 0, completed.stderr)
    require(completed.stdout.strip() ==
            'PASS: predecessor core wait precedes item acquisition and leaves no accepted work',
            'predecessor wait observation absent')


def check(database, source=ROOT):
    inspection = database.command('inspect', database.container)
    require(inspection.returncode == 0, inspection.stderr)
    objects = json.loads(inspection.stdout)
    require(isinstance(objects, list) and len(objects) == 1 and isinstance(objects[0], dict),
            'expected one inspected container')
    inspected = objects[0]
    validate_container(inspected)
    identity = inspected.get('Id')
    require(isinstance(identity, str) and re.fullmatch(r'[0-9a-f]{64}', identity),
            'container identity unavailable')
    # Resolve a supplied name once. Every write/copy then uses the immutable ID;
    # replacing that name cannot transfer the checker to a different database.
    database.container = identity
    database.sql(FRESH)
    copied = database.command('cp', str(source / 'storage/postgresql'),
                              database.container + ':' + DESTINATION)
    require(copied.returncode == 0, copied.stderr)
    fixture = '/opt/kehila-work-item-storage.sql'
    observer = '/opt/kehila-work-item-storage-state.sql'
    history = '/opt/kehila-work-item-history.sql'
    history_observer = '/opt/kehila-work-item-history-state.sql'
    authority = '/opt/kehila-work-item-authority.sql'
    pending = '/opt/kehila-work-item-pending.sql'
    pending_observer = '/opt/kehila-work-item-pending-state.sql'
    discovery = '/opt/kehila-work-item-discovery.sql'
    discovery_observer = '/opt/kehila-work-item-discovery-state.sql'
    reconciliation = '/opt/kehila-work-item-reconciliation.sql'
    reconciliation_observer = '/opt/kehila-work-item-reconciliation-state.sql'
    save = '/opt/kehila-work-item-save.sql'
    save_observer = '/opt/kehila-work-item-save-state.sql'
    for name, destination in (('work_item_storage.sql', fixture), ('work_item_storage_state.sql', observer),
                              ('work_item_history.sql', history), ('work_item_history_state.sql', history_observer),
                              ('work_item_authority.sql', authority),
                              ('work_item_pending.sql', pending), ('work_item_pending_state.sql', pending_observer),
                              ('work_item_project_fixture.sql', '/opt/kehila-work-item-project-fixture.sql'),
                              ('work_item_discovery.sql', discovery), ('work_item_discovery_state.sql', discovery_observer),
                              ('work_item_reconciliation.sql', reconciliation),
                              ('work_item_reconciliation_state.sql', reconciliation_observer),
                              ('work_item_save.sql', save), ('work_item_save_state.sql', save_observer),
                              ('work_item_core_wait.sh', '/opt/kehila-work-item-core-wait.sh')):
        copied = database.command('cp', str(source / 'tests/integration' / name),
                                  database.container + ':' + destination)
        require(copied.returncode == 0, copied.stderr)
    database.sql(file=DESTINATION + '/install_product.sql')
    # Fixed synthetic verifier, independently specified for structural enrollment.
    # No runtime password verification or external corpus claim is made here.
    database.sql("SET ROLE kehila_auth_admin; SELECT * FROM kehila_auth.enroll("
        "'installation','owner','storage-enrollment',"
        "'$argon2id$v=19$m=65536,t=3,p=1$AQEBAQEBAQEBAQEBAQEBAQ$"
        "6d50UfWIhEf6nFrBV/eh7X/wOi7ToEKiEY6e0UdWGa8'); RESET ROLE;")
    database.sql(INSTALLED)
    for function in ('require_product_operation_success(1)',
                     'initialize_project_operational_head()'):
        database.sql('SET ROLE kehila_auth_runtime; SELECT kehila.' + function + ';',
            expected_state='42501',
            expected_message='permission denied for function ' + function.split('(')[0])
    database.sql("SET ROLE kehila_auth_runtime; INSERT INTO kehila.project_operational_heads "
        "VALUES ('forbidden',0,1);", expected_state='42501',
        expected_message='permission denied for table project_operational_heads')
    database.sql(file=fixture)
    # A separate backend observes committed retained identity and head agreement.
    database.sql(file=observer)
    database.sql(file=history)
    database.sql(file=history_observer)
    database.sql(file=authority)
    database.sql(file=pending)
    database.sql(file=pending_observer)
    database.sql(file=discovery)
    database.sql(file=discovery_observer)
    database.sql(file=reconciliation)
    database.sql(file=reconciliation_observer)
    database.sql(file=save)
    check_core_wait(database)
    database.sql(file=save_observer)
    print('PASS: owner-only WorkItem structure, direct edit, retained identity and independent committed observer')


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
    check(DatabaseCheck(podman, args.disposable_container))


if __name__ == '__main__':
    sys.exit(run_main(main, 'ERR_WORK_ITEM_STORAGE_CHECK'))
