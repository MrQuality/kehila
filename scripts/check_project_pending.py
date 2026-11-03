#!/usr/bin/env python3
"""Check product pending SQL in an already-owned fresh isolated PostgreSQL container.

This executes structural fixtures as a separate runtime login. It neither starts
services nor qualifies Rust authentication, typed reconstruction or concurrency.
"""
import argparse
import json
import re
from pathlib import Path

from check_postgresql import DatabaseCheck, DESTINATION, preflight, require, validate_container
from cleanup_errors import run_main

ROOT = Path(__file__).resolve().parents[1]


def profile(check, expected):
    query = (ROOT / 'src/io/kehila_authority/src/postgres/profile.sql').read_text(encoding='utf-8')
    value = 'true' if expected else 'false'
    check.sql('SET ROLE kehila_auth_runtime; CREATE TEMP VIEW selected_profile(allowed) AS\n' + query +
              '\nDO $$ BEGIN IF (SELECT allowed FROM selected_profile) IS DISTINCT FROM ' + value +
              " THEN RAISE EXCEPTION 'unexpected startup capability profile'; END IF; END $$;",
              user='kehila_auth_runtime_login')


def profile_denials(check):
    profile(check, True)
    cases = [
        ('GRANT UPDATE ON kehila.actors TO kehila_auth_runtime',
         'REVOKE UPDATE ON kehila.actors FROM kehila_auth_runtime'),
        ('GRANT UPDATE ON kehila.operations TO kehila_auth_runtime_login',
         'REVOKE UPDATE ON kehila.operations FROM kehila_auth_runtime_login'),
        ('GRANT UPDATE ON SEQUENCE kehila_auth.pending_projects_request_id_seq TO kehila_auth_runtime',
         'REVOKE UPDATE ON SEQUENCE kehila_auth.pending_projects_request_id_seq FROM kehila_auth_runtime'),
        ('GRANT kehila_auth_runtime TO kehila_auth_runtime_login WITH ADMIN OPTION',
         'REVOKE ADMIN OPTION FOR kehila_auth_runtime FROM kehila_auth_runtime_login'),
        ('GRANT kehila_auth_runtime TO kehila_auth_runtime_login WITH INHERIT TRUE',
         'GRANT kehila_auth_runtime TO kehila_auth_runtime_login WITH INHERIT FALSE'),
        ('ALTER FUNCTION kehila_auth.trusted_millis() OWNER TO kehila_auth_runtime_login',
         'ALTER FUNCTION kehila_auth.trusted_millis() OWNER TO kehila_owner'),
        ('ALTER FUNCTION kehila_auth.trusted_millis() OWNER TO kehila_creator',
         'ALTER FUNCTION kehila_auth.trusted_millis() OWNER TO kehila_owner'),
        ('GRANT EXECUTE ON FUNCTION kehila_auth.recover(kehila.ident,kehila_auth.verifier,kehila_auth.public_id) TO kehila_auth_runtime_login',
         'REVOKE EXECUTE ON FUNCTION kehila_auth.recover(kehila.ident,kehila_auth.verifier,kehila_auth.public_id) FROM kehila_auth_runtime_login'),
    ]
    for altered, restored in cases:
        check.sql(altered + ';')
        profile(check, False)
        check.sql(restored + ';')
        profile(check, True)


def read_authority(check):
    role = 'SET ROLE kehila_auth_runtime; '
    statement = "SELECT * FROM kehila_auth.read_authority('installation',1,1);"
    check.sql(role + statement, expected_state='25000', user='kehila_auth_runtime_login')
    check.sql(role + 'BEGIN ISOLATION LEVEL REPEATABLE READ READ WRITE; ' + statement,
              expected_state='25000', user='kehila_auth_runtime_login')
    begin = role + 'BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY; '
    check.sql(begin + "SELECT * FROM kehila_auth.read_authority('installation',2,1);",
              expected_state='55000', user='kehila_auth_runtime_login')
    check.sql(begin + "SELECT * FROM kehila_auth.read_authority('installation',1,2);",
              expected_state='55000', user='kehila_auth_runtime_login')
    check.sql("SET ROLE kehila_owner; UPDATE kehila.actors SET can_create_project=false WHERE actor_id='owner';")
    checked = "DO $$ DECLARE a kehila_auth.authority; BEGIN " \
              "a := kehila_auth.read_authority('installation',1,1); " \
              "IF a.owner_actor_id IS DISTINCT FROM 'owner' THEN RAISE EXCEPTION 'wrong read owner'; " \
              "END IF; END $$; ROLLBACK;"
    check.sql(begin + checked, user='kehila_auth_runtime_login')
    check.sql("SET ROLE kehila_owner; UPDATE kehila.actors SET can_create_project=true,active=false WHERE actor_id='owner';")
    check.sql(begin + statement, expected_state='55000', user='kehila_auth_runtime_login')
    check.sql("SET ROLE kehila_owner; UPDATE kehila.actors SET active=true WHERE actor_id='owner';")
    check.sql(begin + checked, user='kehila_auth_runtime_login')


def run(check):
    inspected = check.command('inspect', check.container)
    require(inspected.returncode == 0, inspected.stderr)
    validate_container(json.loads(inspected.stdout)[0])
    check.sql("DO $$ BEGIN IF EXISTS (SELECT FROM pg_namespace WHERE nspname IN "
              "('kehila','kehila_auth')) OR EXISTS (SELECT FROM pg_roles WHERE rolname IN "
              "('kehila_owner','kehila_creator','kehila_auth_admin','kehila_auth_runtime',"
              "'kehila_auth_runtime_login')) THEN RAISE EXCEPTION 'fresh product database required'; "
              "END IF; END $$;")
    copied = check.command('cp', str(ROOT / 'storage/postgresql'),
                           check.container + ':' + DESTINATION)
    require(copied.returncode == 0, copied.stderr)
    installer = (ROOT / 'storage/postgresql/install_product.sql').read_text(encoding='utf-8')
    require(installer.count('COMMIT;') == 1, 'one atomic product install boundary required')
    failed = installer.replace('COMMIT;', 'SELECT 1/0;\nCOMMIT;')
    check.sql('\\cd ' + DESTINATION + '\n' + failed, expected_state='22012')
    check.sql("DO $$ BEGIN IF EXISTS (SELECT FROM pg_namespace WHERE nspname IN "
              "('kehila','kehila_auth')) OR EXISTS (SELECT FROM pg_roles WHERE rolname IN "
              "('kehila_owner','kehila_creator','kehila_auth_admin','kehila_auth_runtime')) "
              "THEN RAISE EXCEPTION 'product rollback left objects'; END IF; END $$;")
    check.sql(file=DESTINATION + '/install_product.sql')
    check.sql(file=DESTINATION + '/install_product.sql', expected_state='P0001',
              expected_message='fresh dedicated database')
    check.sql("CREATE ROLE kehila_auth_runtime_login LOGIN NOINHERIT NOSUPERUSER "
              "NOCREATEDB NOCREATEROLE NOREPLICATION NOBYPASSRLS; "
              "GRANT kehila_auth_runtime TO kehila_auth_runtime_login; "
              "SET ROLE kehila_auth_admin; "
              "SELECT * FROM kehila_auth.enroll('installation','owner','structural-enroll',"
              "'synthetic-verifier-not-password-qualification');")
    fixture = (ROOT / 'tests/integration/project_pending.sql').read_text(encoding='utf-8')
    output = check.sql(fixture, user='kehila_auth_runtime_login')
    require('PASS: product pending structural capabilities and atomic success' in output,
            'structural completion marker missing')
    profile_denials(check)
    read_authority(check)
    check.sql("DO $$ BEGIN IF EXISTS (SELECT FROM pg_stat_activity WHERE "
              "usename='kehila_auth_runtime_login' AND pid<>pg_backend_pid()) "
              "OR EXISTS (SELECT FROM pg_locks WHERE locktype='advisory' "
              "AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) "
              "THEN RAISE EXCEPTION 'structural runtime connection/lock remains'; END IF; END $$;")
    print('PASS: product install rollback/reinstall, actual-role pending capabilities, '
          'atomic completion, token scope and bounded quota/page fixtures (PostgreSQL 16)')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--disposable-container', required=True)
    parser.add_argument('--podman-connection')
    args = parser.parse_args()
    for name in (args.disposable_container, args.podman_connection):
        require(name is None or re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', name),
                'invalid container/connection name')
    preflight()
    podman = ['podman']
    if args.podman_connection:
        podman += ['--connection', args.podman_connection]
    run(DatabaseCheck(podman, args.disposable_container))


if __name__ == '__main__':
    raise SystemExit(run_main(main, 'PROJECT_PENDING_FAILED'))
