#!/usr/bin/env python3
"""Opt-in fresh selection structure proof in a supplied disposable Podman DB.

The caller owns lifecycle, headroom, deadlines and cleanup. This does not compile,
start a service, upgrade data or qualify authenticated selection/browser behavior.
"""
import argparse
import re
import sys

from check_postgresql import DatabaseCheck, preflight, require
from check_work_item_storage import ROOT, check as work_item_structure
from cleanup_errors import run_main


def check_runtime_profile(database, source):
    # Run the actual serving predicate under its real login/role identities.
    # Each isolated transaction rolls back the single altered capability.
    profile = (source / 'src/io/kehila_authority/src/postgres/profile.sql').read_text(
        encoding='utf-8').strip().removesuffix(';')
    database.sql('CREATE ROLE kehila_auth_runtime_login LOGIN NOINHERIT NOSUPERUSER '
                 'NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS; '
                 'GRANT kehila_auth_runtime TO kehila_auth_runtime_login;')
    cases = (
        ('', True),
        ('GRANT UPDATE ON kehila.current_work_selections TO kehila_auth_runtime;', False),
        ('GRANT INSERT ON kehila.selection_events TO kehila_auth_runtime;', False),
        ('REVOKE SELECT ON kehila.current_work_selections FROM kehila_auth_runtime;', False),
        ('REVOKE SELECT ON kehila.selection_events FROM kehila_auth_runtime;', False),
        ('GRANT EXECUTE ON FUNCTION kehila_auth.selection_fence(kehila_auth.authority) '
         'TO kehila_auth_runtime;', False),
        ('REVOKE EXECUTE ON FUNCTION kehila_auth.reconcile_selection('
         'kehila_auth.authority,kehila.operation_token,bigint,bytea) '
         'FROM kehila_auth_runtime;', False),
        ('ALTER FUNCTION kehila_auth.save_selection(kehila_auth.authority,'
         'kehila.operation_token,kehila.u64,bytea,jsonb) OWNER TO kehila_auth_admin;', False),
        ('ALTER TABLE kehila.selection_events DISABLE TRIGGER selection_success_mapping;', False),
        ('ALTER TABLE kehila.current_work_selections '
         'DISABLE TRIGGER selection_current_boundary;', False),
        ('DROP TRIGGER selection_success_mapping ON kehila.selection_events; '
         'CREATE CONSTRAINT TRIGGER selection_success_mapping AFTER INSERT '
         'ON kehila.selection_events DEFERRABLE INITIALLY DEFERRED FOR EACH ROW '
         'WHEN (false) EXECUTE FUNCTION kehila.check_selection_operation_success();', False),
    )
    for index, (change, expected) in enumerate(cases):
        database.sql(
            'BEGIN; ' + change +
            ' SET SESSION AUTHORIZATION kehila_auth_runtime_login; '
            'SET ROLE kehila_auth_runtime; DO $profile$ DECLARE admitted boolean; BEGIN '
            'admitted := (' + profile + '); IF admitted IS DISTINCT FROM ' +
            str(expected).lower() + " THEN RAISE EXCEPTION 'selection profile case " +
            str(index) + "'; END IF; END; $profile$; ROLLBACK;")
    # A separate clean backend proves the negative controls left a usable profile.
    database.sql(
        'SET ROLE kehila_auth_runtime; '
        'DO $profile$ BEGIN IF (' + profile + ") IS DISTINCT FROM true THEN "
        "RAISE EXCEPTION 'restored selection profile'; END IF; END; $profile$;",
        user='kehila_auth_runtime_login')


def check(database, source=ROOT):
    # Reuse the existing fresh-database and immutable-container-ID admission.
    # Its independent item observers remain controls for unchanged item behavior.
    work_item_structure(database, source)
    paths = (
        ('selection_storage.sql', '/opt/kehila-selection-storage.sql'),
        ('selection_storage_state.sql', '/opt/kehila-selection-storage-state.sql'),
        ('selection_authority.sql', '/opt/kehila-selection-authority.sql'),
        ('selection_authority_state.sql', '/opt/kehila-selection-authority-state.sql'),
    )
    for name, destination in paths:
        copied = database.command('cp', str(source / 'tests/integration' / name),
                                  database.container + ':' + destination)
        require(copied.returncode == 0, copied.stderr)
    database.sql(file=paths[0][1])
    # A new psql backend supplies an independent committed-state observation.
    database.sql(file=paths[1][1])
    database.sql(file=paths[2][1])
    database.sql(file=paths[3][1])
    check_runtime_profile(database, source)
    print('PASS: fresh selection no-op structure, capability restrictions and independent committed state')


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
    sys.exit(run_main(main, 'ERR_SELECTION_STORAGE_CHECK'))
