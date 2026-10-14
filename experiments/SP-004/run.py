"""Opt-in serial A05 grant-management experiment; caller owns the disposable container."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[2]
SOURCE = Path(__file__).resolve().parent
sys.path.insert(0, str(ROOT / 'scripts'))
import check_postgresql as native

CHANGE = "sp004.change_grant('actor','actor','created',{}, {}, '{}')"


def bounded(statement):
    return ("SET statement_timeout='10s'; SET lock_timeout='2s'; "
            "SET idle_in_transaction_session_timeout='10s';\n" + statement)


def verify(database, condition, message):
    # Only constant experiment-owned SQL and messages are passed here.
    database.sql(bounded("DO $$ BEGIN IF (" + condition + ") IS DISTINCT FROM true THEN "
                         "RAISE EXCEPTION '" + message + "'; END IF; END $$;"))


def run(database):
    database.run()  # Fresh-container validation and native baseline must pass first.
    database.sql(bounded("INSERT INTO kehila.project_grant_events "
        "(project_id,actor_id,version,access_profile,active,granted_by,"
        "granting_operation_row_id,command_family) SELECT project_id,actor_id,2,"
        "access_profile,false,granted_by,granting_operation_row_id,command_family "
        "FROM kehila.project_grant_events WHERE project_id='created';"),
        expected_state='23514', expected_message='slice1_grant_event')
    database.sql((SOURCE / 'overlay.sql').read_text(encoding='utf-8'))

    serving = 'foundation_serving'
    manager = 'sp004_manager_test'
    database.sql(bounded("SELECT " + CHANGE.format(1, 'false', 'forbidden') + ';'),
                 user=serving, expected_state='42501',
                 expected_message='permission denied for function change_grant')
    database.sql(bounded("UPDATE kehila.project_grants SET active=false;"),
                 user=manager, expected_state='42501',
                 expected_message='permission denied for table project_grants')
    database.sql(bounded("INSERT INTO kehila.operations "
        "(actor_id,command_family,target_kind,target_project_id,token,"
        "request_codec_version,request_sha256,required_grant,grant_policy_version,"
        "replay_origin_ms,replay_deadline_ms) VALUES ('actor','sp004_grant_admin',"
        "'project','created','forbidden',1,decode(repeat('00',32),'hex'),"
        "'sp004_grant_admin',1,0,7776000000);"), user=serving,
        expected_state='42501', expected_message='management writes require the restricted helper')
    database.sql(bounded("INSERT INTO kehila.project_grant_events "
        "(project_id,actor_id,version,access_profile,active,granted_by,"
        "granting_operation_row_id,command_family) SELECT project_id,actor_id,2,"
        "access_profile,false,granted_by,granting_operation_row_id,'sp004_grant_admin' "
        "FROM kehila.project_grant_events WHERE project_id='created';"), user=serving,
        expected_state='42501', expected_message='management writes require the restricted helper')
    database.sql(bounded("SELECT " + CHANGE.format(2, 'false', 'stale') + ';'),
                 user=manager, expected_state='40001', expected_message='stale grant version')
    database.sql(bounded("SELECT " + CHANGE.format(1, 'true', 'noop') + ';'),
                 user=manager, expected_state='22023', expected_message='grant change is a no-op')

    database.sql(bounded("BEGIN; SELECT " + CHANGE.format(1, 'false', 'rollback') +
                         '; SET CONSTRAINTS ALL IMMEDIATE; ROLLBACK;'), user=manager)
    verify(database,
           "(SELECT active AND version=1 FROM kehila.project_grants "
           "WHERE project_id='created' AND actor_id='actor') AND NOT EXISTS "
           "(SELECT FROM kehila.operations WHERE command_family='sp004_grant_admin') "
           "AND (SELECT count(*)=1 FROM kehila.project_grant_events WHERE project_id='created')",
           'management rollback left state')

    database.sql(bounded("SELECT " + CHANGE.format(1, 'false', 'revoke') + ';'), user=manager)
    verify(database,
           "EXISTS (SELECT FROM kehila.project_grants g "
           "JOIN kehila.project_grant_events e USING(project_id,actor_id,version) "
           "JOIN kehila.operations o ON o.operation_row_id=e.granting_operation_row_id "
           "JOIN kehila.operation_payloads p USING(operation_row_id) "
           "WHERE g.project_id='created' AND g.actor_id='actor' AND g.version=2 "
           "AND NOT g.active AND e.active=g.active AND e.access_profile=g.access_profile "
           "AND o.actor_id=e.granted_by AND o.target_project_id=g.project_id "
           "AND o.command_family=e.command_family AND o.command_family='sp004_grant_admin' "
           "AND o.required_grant=o.command_family AND o.grant_policy_version=1 "
           "AND o.request_sha256=sha256(p.request_bytes) "
           "AND convert_from(p.request_bytes,'UTF8')::jsonb = "
           "jsonb_build_array('sp004_grant_admin_v1','actor','actor','created','1',false) "
           "AND p.result=jsonb_build_object('kind','sp004_grant_change','version','2',"
           "'actor','actor','project','created','active',false))",
           'management attribution or payload mismatch')
    database.sql(bounded("SELECT " + CHANGE.format(2, 'true', 'restore') + ';'), user=manager)

    # Deliberate corruption stays in an administrator transaction and rolls back
    # when psql exits on the expected error. Serving never repairs it implicitly.
    database.sql(bounded("BEGIN; UPDATE kehila.project_grants SET active=false "
        "WHERE project_id='created'; SET LOCAL ROLE sp004_manager; SELECT " +
        CHANGE.format(3, 'false', 'corrupt') + ';'), expected_state='XX001',
        expected_message='current grant and latest history disagree')
    verify(database,
           "(SELECT active AND version=3 FROM kehila.project_grants WHERE project_id='created') "
           "AND (SELECT count(*)=3 FROM kehila.project_grant_events WHERE project_id='created') "
           "AND (SELECT count(*)=2 FROM kehila.operations WHERE command_family='sp004_grant_admin')",
           'corruption probe changed committed state')
    database.sql(bounded("UPDATE kehila.project_grant_events SET active=false;"), user=manager,
                 expected_state='42501', expected_message='permission denied for table project_grant_events')
    database.sql(bounded("DELETE FROM kehila.project_grant_events;"),
                 expected_state='23514', expected_message='creation history is append-only')

    return {
        'scope': 'SP-004 A05 serial management subset',
        'outcome': 'passed',
        'platform': platform.system(),
        'sources': {path.relative_to(ROOT).as_posix(): hashlib.sha256(path.read_bytes()).hexdigest()
                    for path in [SOURCE / 'run.py', SOURCE / 'overlay.sql',
                                 ROOT / 'scripts/check_postgresql.py',
                                 *sorted((ROOT / 'storage/postgresql').glob('*.sql'))]},
        'not_qualified': ['real authentication/session/channel mechanisms',
                          'concurrent revocation schedules', 'read/projection races',
                          'generic original-scope replay', 'production migration/adapter'],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--disposable-container', required=True)
    parser.add_argument('--podman-connection')
    args = parser.parse_args()
    for name in (args.disposable_container, args.podman_connection):
        native.require(name is None or re.fullmatch(r'[A-Za-z0-9][A-Za-z0-9_.-]{0,127}', name),
                       'invalid container or connection name')
    native.preflight()
    podman = ['podman']
    if args.podman_connection:
        podman += ['--connection', args.podman_connection]
    result = run(native.DatabaseCheck(podman, args.disposable_container))
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    try:
        main()
    except (OSError, ValueError, KeyError, subprocess.SubprocessError) as error:
        print('ERR_SP004_CHECK: ' + str(error), file=sys.stderr)
        sys.exit(3)
