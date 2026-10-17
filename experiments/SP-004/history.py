"""Full experimental audits and bounded history-size observations; native evidence only."""
import json
from pathlib import Path
import time

SOURCE = Path(__file__).resolve().parent


def record(output):
    if len(output.encode('utf-8')) > 65536:
        raise ValueError('history evidence exceeds 64 KiB')
    rows = [line.strip() for line in output.splitlines() if line.lstrip().startswith('{')]
    if len(rows) != 1:
        raise ValueError('expected exactly one history evidence record')
    return json.loads(rows[0])


def state(database):
    return record(database.sql("\\pset tuples_only on\n\\pset format unaligned\n"
        "SELECT json_build_object('grant',(SELECT row_to_json(g) FROM kehila.project_grants g "
        "WHERE project_id='created' AND actor_id='actor'), 'events',"
        "(SELECT count(*) FROM kehila.project_grant_events WHERE project_id='created'),"
        "'operations',(SELECT count(*) FROM kehila.operations));"))


def run(database):
    database.sql((SOURCE / 'history.sql').read_text(encoding='utf-8'))
    original = state(database)
    database.sql("SET ROLE kehila_owner; SELECT sp004.audit_grant('created','actor');")
    corruptions = [
        ("ALTER TABLE kehila.project_grant_events DISABLE TRIGGER grant_history_immutable; "
         "DELETE FROM kehila.project_grant_events WHERE project_id='created' AND version=2;",
         'audit: grant history gap'),
        ("SET LOCAL session_replication_role=replica; "
         "UPDATE kehila.project_grant_events SET granted_by='actor' "
         "WHERE project_id='created' AND version=4; SET LOCAL session_replication_role=origin;",
         'audit: grant attribution mismatch'),
        ("ALTER TABLE kehila.operation_payloads DISABLE TRIGGER operation_payload_immutable; "
         "UPDATE kehila.operation_payloads SET request_bytes='\\x00' WHERE operation_row_id="
         "(SELECT granting_operation_row_id FROM kehila.project_grant_events "
         "WHERE project_id='created' AND version=2);", 'audit: grant payload mismatch'),
    ]
    for corruption, diagnostic in corruptions:
        database.sql("BEGIN; SET LOCAL statement_timeout='20s'; " + corruption +
                     " SELECT sp004.audit_grant('created','actor');", expected_state='XX001',
                     expected_message=diagnostic)
        if state(database) != original:
            raise ValueError('history corruption probe changed committed state')
        database.sql("SET ROLE kehila_owner; SELECT sp004.audit_grant('created','actor');")

    # Mutate the executable audit, then prove the gap rejection is essential.
    source = (SOURCE / 'history.sql').read_text(encoding='utf-8')
    changed = source.replace('IF e.version <> previous_version + 1 THEN', 'IF false THEN')
    if changed == source:
        raise ValueError('history mutation target missing')
    replacement = changed.split('CREATE FUNCTION', 1)[1].split('REVOKE ALL', 1)[0]
    database.sql('BEGIN; SET LOCAL ROLE kehila_owner; CREATE OR REPLACE FUNCTION' +
                 replacement + ' COMMIT;')
    try:
        database.sql('BEGIN; ' + corruptions[0][0] +
                     " DO $$ BEGIN PERFORM sp004.audit_grant('created','actor'); "
                     "RAISE EXCEPTION 'gap mutant accepted corruption'; END $$;",
                     expected_state='P0001', expected_message='gap mutant accepted corruption')
    finally:
        replacement = source.split('CREATE FUNCTION', 1)[1].split('REVOKE ALL', 1)[0]
        database.sql('BEGIN; SET LOCAL ROLE kehila_owner; CREATE OR REPLACE FUNCTION' +
                     replacement + ' COMMIT;')
    if state(database) != original:
        raise ValueError('history mutation changed committed state')
    database.sql('BEGIN; ' + corruptions[0][0] +
                 " SELECT sp004.audit_grant('created','actor');", expected_state='XX001',
                 expected_message='audit: grant history gap')

    measurements = []
    previous = 5  # Serial R03 leaves five coherent events.
    for size in (10, 100, 1000):
        # Each helper call uses the existing attributed operation/payload/event path.
        database.sql("SET statement_timeout='20s'; SET ROLE sp004_manager_test; DO $$ "
            "DECLARE n numeric; BEGIN FOR n IN " + str(previous + 1) + '..' + str(size) +
            " LOOP PERFORM sp004.change_grant('admin_actor','actor','created',n-1,"
            "mod(n,2)=1,'history-'||n::text); END LOOP; END $$;")
        queries = []
        for _ in range(3):
            text = database.sql("\\pset tuples_only on\n\\pset format unaligned\n"
                "EXPLAIN (ANALYZE, FORMAT JSON) SELECT g.version,g.active,e.version,e.active "
                "FROM kehila.project_grants g CROSS JOIN LATERAL "
                "(SELECT version,active,access_profile FROM kehila.project_grant_events "
                "WHERE project_id=g.project_id AND actor_id=g.actor_id ORDER BY version DESC LIMIT 1) e "
                "WHERE g.project_id='created' AND g.actor_id='actor' "
                "AND (g.version,g.active,g.access_profile)=(e.version,e.active,e.access_profile);")
            start, end = text.find('['), text.rfind(']')
            if start < 0 or end < start or len(text.encode('utf-8')) > 65536:
                raise ValueError('missing or oversized history query plan')
            plan = json.loads(text[start:end + 1])[0]
            if plan['Plan']['Actual Rows'] != 1:
                raise ValueError('bounded grant check did not match current/latest')
            queries.append(plan)
        started = time.monotonic()
        audit = record(database.sql("\\pset tuples_only on\n\\pset format unaligned\n"
            "SET statement_timeout='20s'; SET ROLE kehila_owner; SELECT json_build_object("
            "'events',sp004.audit_grant('created','actor'));"))
        if audit != {'events': size}:
            raise ValueError('history audit did not visit every event')
        measurements.append({'events': size, 'current_latest_plans': queries, 'audit': audit,
                             'audit_host_elapsed_seconds': time.monotonic() - started})
        previous = size
    return {'corruptions': [label for _, label in corruptions],
            'gap_mutation_detected': True, 'measurements': measurements}
