"""Reached database authority schedules; structural fixtures, not application routes."""
from pathlib import Path
import time
import uuid

from cleanup_errors import CleanupStack
from history import record
from sessions import Session, SqlFailure, BarrierNotReached, blocked

SOURCE = Path(__file__).resolve().parent


class UnexpectedAuthorityOutcome(ValueError):
    def __init__(self, outcome):
        self.outcome = outcome
        super().__init__('expected current-authority denial, got ' + repr(outcome))


class LockOrderViolation(ValueError):
    def __init__(self, later):
        self.later = later
        super().__init__('earlier actor was not held after reaching the later-actor barrier')


def query(database, expression):
    return record(database.sql("\\pset tuples_only on\n\\pset format unaligned\n"
                               "SELECT jsonb_build_object('value',(" + expression + '));'))['value']


def permission(database, active):
    current = query(database, "SELECT can_create_project FROM kehila.actors WHERE actor_id='actor'")
    if current != active:
        database.sql("SELECT sp004.change_creation('admin_actor','actor'," +
                     str(current).lower() + ',' + str(active).lower() + ",'reset-" + uuid.uuid4().hex + "');",
                     user='sp004_manager_test')


def project_state(database, project):
    fields = []
    for table in ('projects', 'configuration_revisions', 'project_grants', 'project_grant_events',
                  'statuses', 'workflows', 'workflow_statuses', 'workflow_phase_changes',
                  'work_item_types', 'type_workflows', 'fields'):
        fields.append("'" + table + "',(SELECT count(*) FROM kehila." + table +
                      " WHERE project_id='" + project + "')")
    fields.append("'operations',(SELECT count(*) FROM kehila.operations WHERE "
                  "command_family='project_create' AND target_project_id='" + project + "')")
    fields.append("'payloads',(SELECT count(*) FROM kehila.operation_payloads p "
                  "JOIN kehila.operations o USING(operation_row_id) "
                  "WHERE o.command_family='project_create' AND o.target_project_id='" + project + "')")
    return query(database, 'SELECT json_build_object(' + ','.join(fields) + ')')


def check_project(database, project, exists, grant_events=1, actor='actor'):
    counts = project_state(database, project)
    expected = {key: (2 if key in ('statuses', 'workflow_statuses') else 1) if exists else 0
                for key in counts}
    if exists:
        expected['project_grant_events'] = grant_events
    if counts != expected:
        raise ValueError('partial or duplicate creation state: ' + repr(counts))
    if exists and not query(database,
        "SELECT p.created_by='" + actor + "' AND p.configuration_revision=1 "
        "AND o.actor_id=p.created_by AND o.command_family='project_create' "
        "AND o.required_grant='project_create' AND o.grant_policy_version=1 "
        "AND o.request_codec_version=1 AND NOT o.payload_retired "
        "AND x.request_bytes=convert_to(jsonb_build_array('sp004_creation_fixture',p.created_by,p.project_id,o.token)::text,'UTF8') "
        "AND o.request_sha256=sha256(x.request_bytes) AND x.result_codec_version=1 "
        "AND x.result=jsonb_build_object('kind','sp004_creation_fixture','project',p.project_id) "
        "AND r.actor_id=p.created_by AND r.operation_row_id=o.operation_row_id "
        "AND e.granted_by=p.created_by AND e.granting_operation_row_id=o.operation_row_id "
        "AND e.command_family='project_create' AND e.active AND e.access_profile='initial_owner' "
        "FROM kehila.projects p JOIN kehila.operations o ON o.operation_row_id=p.creation_operation_row_id "
        "JOIN kehila.operation_payloads x USING(operation_row_id) "
        "JOIN kehila.configuration_revisions r ON r.project_id=p.project_id AND r.revision=1 "
        "JOIN kehila.project_grant_events e ON e.project_id=p.project_id AND e.actor_id=p.created_by AND e.version=1 "
        "WHERE p.project_id='" + project + "'"):
        raise ValueError('creation fixture request/result/actor attribution mismatch')
    return counts


def denial(participant, scope):
    try:
        result = participant.wait()
    except SqlFailure as error:
        if (error.state, error.primary) != ('42501', scope + ' authority denied'):
            raise
        return 'denied'
    raise UnexpectedAuthorityOutcome(result)


def schedule(database, *, replay, command_first, rollback=False, project_scope=False, label):
    permission(database, True)
    project, token = 'case-' + label, 'command-' + label
    create = "SELECT sp004.fixture_create('actor','" + project + "','" + token + "');"
    command = ("SELECT sp004.fixture_save('actor','" + project + "','" + token + "');"
               if project_scope else create)
    if replay or project_scope:
        database.sql(create, user='foundation_serving')
    if replay and project_scope:
        database.sql(command, user='foundation_serving')
    revoke = ("SELECT sp004.change_grant('admin_actor','actor','" + project + "',1,false,'revoke-" + label + "');"
              if project_scope else
              "SELECT sp004.change_creation('admin_actor','actor',true,false,'revoke-" + label + "');")
    scope = 'project' if project_scope else 'creation'
    success = 'replayed' if replay else ('saved' if project_scope else 'created')
    with CleanupStack() as owned:
        first = owned.enter_context(Session(database, 'foundation_serving' if command_first else 'sp004_manager_test'), resource='first participant')
        second = owned.enter_context(Session(database, 'sp004_manager_test' if command_first else 'foundation_serving'), resource='second participant')
        observer = owned.enter_context(Session(database, 'postgres'), resource='lock observer')
        first.execute('BEGIN;')
        first_outcome = first.execute(command if command_first else revoke)
        if command_first and first_outcome != success:
            raise ValueError('unexpected command-first outcome: ' + first_outcome)
        if not command_first and first_outcome != ('2' if project_scope else 'f'):
            raise ValueError('unexpected revocation-first outcome: ' + first_outcome)
        second.execute('BEGIN;')
        second.send(revoke if command_first else command)
        barrier = blocked(observer, second, first)
        first.execute('ROLLBACK;' if rollback else 'COMMIT;')
        if command_first or rollback:
            second_outcome = second.wait()
            if command_first and second_outcome != ('2' if project_scope else 'f'):
                raise ValueError('unexpected concurrent revocation outcome: ' + second_outcome)
            if not command_first and second_outcome != success:
                raise ValueError('rollback retry did not reload current authority')
            second.execute('COMMIT;')
        else:
            second_outcome = denial(second, scope)
    exists = replay or project_scope or (command_first and not rollback) or (not command_first and rollback)
    grant_revoked = project_scope and (command_first or not rollback)
    state = check_project(database, project, exists, 2 if grant_revoked else 1)
    if command_first and rollback:
        # A fresh retry must reload the now-committed revocation, not reuse first's verdict.
        database.sql(command, user='foundation_serving', expected_state='42501',
                     expected_message=scope + ' authority denied')
    if project_scope:
        expected_version = 2 if grant_revoked else 1
        if not query(database, "SELECT version=" + str(expected_version) + ' AND active=' + str(not grant_revoked).lower() +
                     " FROM kehila.project_grants WHERE project_id='" + project + "' AND actor_id='actor'"):
            raise ValueError('project grant commit/rollback mismatch')
        if not query(database, "SELECT can_create_project FROM kehila.actors WHERE actor_id='actor'"):
            raise ValueError('project revocation incorrectly changed creation authority')
        if not query(database, "SELECT active FROM kehila.project_grants "
                     "WHERE project_id='scope-control' AND actor_id='actor'"):
            raise ValueError('project revocation crossed project boundary')
        saved = query(database, "SELECT count(*) FROM sp004.project_fixture_operations WHERE project_id='" + project + "'")
        expected_saved = int(replay or (command_first and not rollback) or (not command_first and rollback))
        if saved != expected_saved:
            raise ValueError('project fixture replay/rollback state mismatch')
        database.sql("DO $$ BEGIN IF sp004.fixture_save('actor','scope-control','control-" + label + "') <> 'saved' "
                     "OR sp004.fixture_create('actor','" + project + "','" + token + "') <> 'replayed' THEN "
                     "RAISE EXCEPTION 'project revocation crossed command scope'; END IF; END $$;",
                     user='foundation_serving')
        state = check_project(database, project, True, 2 if grant_revoked else 1)
    elif query(database, "SELECT can_create_project FROM kehila.actors WHERE actor_id='actor'") != (not command_first and rollback):
        raise ValueError('creation permission commit/rollback state mismatch')
    if not project_scope:
        changes = query(database, "SELECT count(*) FROM sp004.creation_permission_changes WHERE token='revoke-" + label +
                        "' AND manager='admin_actor' AND subject='actor' AND previous AND NOT active")
        if changes != int(command_first or not rollback):
            raise ValueError('creation management attribution/rollback mismatch')
    return dict(label=label, replay=replay, command_first=command_first, rollback=rollback,
                project_scope=project_scope, barrier=barrier, first=first_outcome,
                second=second_outcome, state=state)


def lock_order(database, manager, subject, project, label):
    """Gate the later actor; the helper must already hold the earlier actor."""
    with CleanupStack() as owned:
        gate = owned.enter_context(Session(database, 'foundation_serving'), resource='later-actor gate')
        worker = owned.enter_context(Session(database, 'sp004_manager_test'), resource='management participant')
        probe = owned.enter_context(Session(database, 'foundation_serving'), resource='earlier-actor probe')
        observer = owned.enter_context(Session(database, 'postgres'), resource='order observer')
        gate.execute("BEGIN; SELECT actor_id FROM kehila.lock_actor('admin_actor');")
        worker.execute('BEGIN;')
        worker.send("SELECT sp004.change_grant('" + manager + "','" + subject + "','" + project + "',1,false,'" + label + "');")
        later = blocked(observer, worker, gate)
        probe.execute('BEGIN;')
        probe.send("SELECT actor_id FROM kehila.lock_actor('actor');")
        try:
            earlier = blocked(observer, probe, worker)
        except BarrierNotReached as error:
            raise LockOrderViolation(later) from error
        gate.execute('COMMIT;')
        if worker.wait() != '2':
            raise ValueError('management successor mismatch')
        worker.execute('ROLLBACK;')
        if probe.wait() != 'actor':
            raise ValueError('earlier actor probe mismatch')
        probe.execute('ROLLBACK;')
    if not query(database, "SELECT active AND version=1 FROM kehila.project_grants WHERE project_id='" + project + "' AND actor_id='" + subject + "'"):
        raise ValueError('lock-order rollback changed grant')
    check_project(database, project, True, actor=subject)
    if query(database, "SELECT count(*) FROM kehila.operations WHERE command_family='sp004_grant_admin' AND target_project_id='" + project + "'") != 0:
        raise ValueError('lock-order rollback left management operations')
    return {'manager': manager, 'subject': subject, 'later': later, 'earlier': earlier}


def competing_management(database, rollback):
    project = 'management-' + ('rollback' if rollback else 'commit')
    database.sql("SELECT sp004.fixture_create('actor','" + project + "','create');", user='foundation_serving')
    def change(manager, expected, active, token):
        return "SELECT sp004.change_grant('" + manager + "','actor','" + project + "'," + str(expected) + ',' + str(active).lower() + ",'" + token + "');"
    with CleanupStack() as owned:
        first = owned.enter_context(Session(database, 'sp004_manager_test'), resource='first grant writer')
        second = owned.enter_context(Session(database, 'sp004_manager_test'), resource='second grant writer')
        observer = owned.enter_context(Session(database, 'postgres'), resource='grant contention observer')
        first.execute('BEGIN; ' + change('actor', 1, False, 'first'))
        second.execute('BEGIN;')
        second.send(change('admin_actor', 1, False, 'second'))
        barrier = blocked(observer, second, first)
        first.execute('ROLLBACK;' if rollback else 'COMMIT;')
        if rollback:
            if second.wait() != '2':
                raise ValueError('rollback did not allow one management successor')
            second.execute('COMMIT;')
        else:
            try:
                second.wait()
            except SqlFailure as error:
                if (error.state, error.primary) != ('40001', 'stale grant version'):
                    raise
            else:
                raise ValueError('management contention did not reject stale version')
    expected_manager = 'admin_actor' if rollback else 'actor'
    if not query(database, "SELECT g.version=2 AND NOT g.active AND e.granted_by='" + expected_manager +
                 "' AND o.actor_id=e.granted_by AND o.command_family='sp004_grant_admin' "
                 "AND o.request_sha256=sha256(p.request_bytes) "
                 "FROM kehila.project_grants g JOIN kehila.project_grant_events e USING(project_id,actor_id,version) "
                 "JOIN kehila.operations o ON o.operation_row_id=e.granting_operation_row_id "
                 "JOIN kehila.operation_payloads p USING(operation_row_id) WHERE g.project_id='" + project + "' AND g.actor_id='actor'"):
        raise ValueError('management contention attribution/state mismatch')
    check_project(database, project, True, 2)
    database.sql(change('admin_actor', 2, True, 'fresh-retry'), user='sp004_manager_test')
    check_project(database, project, True, 3)
    if not query(database, "SELECT active AND version=3 FROM kehila.project_grants "
                 "WHERE project_id='" + project + "' AND actor_id='actor'"):
        raise ValueError('fresh management retry did not restore the grant')
    if query(database, "SELECT count(*) FROM kehila.operations WHERE command_family='sp004_grant_admin' "
             "AND target_project_id='" + project + "'") != 2:
        raise ValueError('management retry left extra operations')
    return {'rollback': rollback, 'barrier': barrier, 'fresh_retry_version': 3}


def timeout_cleanup(database):
    project = 'timeout-cleanup'
    database.sql("SELECT sp004.fixture_create('actor','" + project + "','create');", user='foundation_serving')
    reached = False
    try:
        with Session(database, 'sp004_manager_test') as participant:
            if participant.execute("BEGIN; SELECT sp004.change_grant('admin_actor','actor','" + project + "',1,false,'timeout');") != '2':
                raise ValueError('uncommitted grant change was not reached')
            if participant.execute("SELECT count(*)=1 FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id) "
                                   "WHERE o.command_family='sp004_grant_admin' AND o.target_project_id='" + project + "' AND o.token='timeout';") != 't':
                raise ValueError('uncommitted management operation was not reached')
            reached = True
            participant.deadline = time.monotonic() + 0.02
            participant.execute('SELECT pg_sleep(1);')
    except TimeoutError as error:
        if str(error) != 'native participant exceeded 60 seconds' or getattr(error, '_cleanup_failures', ()):
            raise
        if not reached:
            raise ValueError('timeout probe did not reach the uncommitted change') from error
    else:
        raise ValueError('native participant timeout was not reached')
    check_project(database, project, True)
    if not query(database, "SELECT active AND version=1 FROM kehila.project_grants WHERE project_id='" + project + "' AND actor_id='actor'"):
        raise ValueError('participant timeout left a committed revocation')
    if query(database, "SELECT count(*) FROM kehila.operations WHERE command_family='sp004_grant_admin' AND target_project_id='" + project + "'") != 0:
        raise ValueError('participant timeout left a management operation')
    return 'deadline propagated; backend/process reaped; grant/event/operation rolled back'


def mutations(database):
    signatures = [
        ('sp004.fixture_create(kehila.ident,kehila.ident,kehila.operation_token)',
         'OR NOT a.can_create_project', '', UnexpectedAuthorityOutcome),
        ('sp004.change_grant(kehila.ident,kehila.ident,kehila.ident,kehila.u64,boolean,kehila.operation_token)',
         'ORDER BY a.actor_id COLLATE "C" FOR NO KEY UPDATE',
         'ORDER BY a.actor_id COLLATE "C" DESC FOR NO KEY UPDATE', LockOrderViolation),
    ]
    detected = []
    for index, (signature, before, after, expected_failure) in enumerate(signatures):
        definition = query(database, "SELECT pg_get_functiondef('" + signature + "'::regprocedure)")
        if definition.count(before) != 1:
            raise ValueError('native authority mutation target missing or ambiguous')
        database.sql(definition.replace(before, after))
        try:
            try:
                if index == 0:
                    schedule(database, replay=False, command_first=False, label='authority-mutant')
                else:
                    lock_order(database, 'admin_actor', 'actor', 'order-subject', 'order-mutant')
            except expected_failure as error:
                if getattr(error, '_cleanup_failures', ()):
                    raise
                if index == 0:
                    if error.outcome != 'created':
                        raise
                    detected.append({'kind': 'creation authority bypass', 'observed': error.outcome})
                else:
                    detected.append({'kind': 'actor lock-order reversal', 'reached_later_barrier': error.later})
            else:
                raise ValueError('native authority mutation survived')
        finally:
            database.sql(definition)
        if index == 0:
            check_project(database, 'case-authority-mutant', False)
            schedule(database, replay=False, command_first=False, label='authority-restored')
        else:
            lock_order(database, 'admin_actor', 'actor', 'order-subject', 'order-restored')
    return detected


def creation_management_boundaries(database):
    inputs = ["'admin_actor'", "'actor'", 'true', 'false', "'null-input'"]
    for index in range(len(inputs)):
        invalid = inputs.copy()
        invalid[index] = 'NULL'
        database.sql('SELECT sp004.change_creation(' + ','.join(invalid) + ');',
                     user='sp004_manager_test', expected_state='22004',
                     expected_message='creation management inputs required')
    for token in ('', 'bad token', 'x' * 129):
        database.sql("SELECT sp004.change_creation('admin_actor','actor',true,false,'" + token + "');",
                     user='sp004_manager_test', expected_state='23514', expected_message='operation_token')
    for manager in ('inactive_manager', 'unprivileged_actor', 'missing_manager'):
        database.sql("SELECT sp004.change_creation('" + manager + "','actor',true,false,'denied');",
                     user='sp004_manager_test', expected_state='42501',
                     expected_message='creation management authority denied')
    for statement, state, message in (
        ("sp004.change_creation('admin_actor','missing_subject',true,false,'missing')", '23503', 'creation subject missing'),
        ("sp004.change_creation('admin_actor','actor',false,true,'stale')", '40001', 'stale creation permission'),
        ("sp004.change_creation('admin_actor','actor',true,true,'noop')", '22023', 'creation permission no-op')):
        database.sql('SELECT ' + statement + ';', user='sp004_manager_test',
                     expected_state=state, expected_message=message)
    for user in ('foundation_serving', 'sp004_manager_test'):
        database.sql("INSERT INTO sp004.creation_permission_changes VALUES ('forged','admin_actor','actor',true,false);",
                     user=user, expected_state='42501', expected_message='permission denied for table creation_permission_changes')
    if query(database, 'SELECT count(*) FROM sp004.creation_permission_changes') != 0:
        raise ValueError('creation management rejection changed state')
    return {'null_inputs': 5, 'invalid_tokens': 3, 'denied_managers': 3,
            'missing_subject': 1, 'stale': 1, 'no_op': 1, 'forged_log_writes': 2}


def run(database):
    # PostgreSQL json_agg(composite) can contain physical newlines. Preserve the
    # failed producer as a native regression; the root JSONB cast above frames
    # one compact JSON record without loosening the consumer's one-record rule.
    multiline = database.sql("\\pset tuples_only on\n\\pset format unaligned\n"
        "SELECT json_build_object('value',(SELECT json_agg(a ORDER BY actor_id) FROM kehila.actors a));")
    try:
        record(multiline)
    except ValueError as error:
        if str(error) != 'expected exactly one history evidence record':
            raise
    else:
        raise ValueError('multiline evidence regression was not reached')
    original_actors = query(database, "SELECT json_agg(a ORDER BY actor_id) FROM kehila.actors a")
    database.sql((SOURCE / 'concurrent.sql').read_text(encoding='utf-8'))
    # Native restricted-role boundaries for the new management capability.
    database.sql("UPDATE kehila.actors SET can_create_project=false;", user='sp004_manager_test',
                 expected_state='42501', expected_message='permission denied for table actors')
    database.sql("SELECT sp004.change_creation('admin_actor','actor',true,false,'forbidden');",
                 user='foundation_serving', expected_state='42501', expected_message='permission denied for function change_creation')
    boundary_cases = creation_management_boundaries(database)
    if query(database, "SELECT json_agg(a ORDER BY actor_id) FROM kehila.actors a") != original_actors:
        raise ValueError('creation management rejection changed actor state')
    database.sql("SELECT sp004.fixture_create('actor','scope-control','scope-control');", user='foundation_serving')
    results = []
    for replay in (False, True):
        for command_first in (True, False):
            prefix = ('replay' if replay else 'create') + ('-first' if command_first else '-revoked')
            for trial in range(10):
                results.append(schedule(database, replay=replay, command_first=command_first, label=prefix + '-' + str(trial)))
            results.append(schedule(database, replay=replay, command_first=command_first, rollback=True, label=prefix + '-rollback'))
            for rollback in (False, True):
                results.append(schedule(database, replay=replay, command_first=command_first,
                                        rollback=rollback, project_scope=True,
                                        label='project-' + prefix + ('-rollback' if rollback else '')))
    permission(database, True)
    database.sql("SELECT sp004.change_creation('actor','admin_actor',false,true,'manager-create');",
                 user='sp004_manager_test')
    for actor, project in (('actor','order-subject'), ('admin_actor','order-manager')):
        database.sql("SELECT sp004.fixture_create('" + actor + "','" + project + "','order');", user='foundation_serving')
    orders = [lock_order(database, 'admin_actor', 'actor', 'order-subject', 'order-normal'),
              lock_order(database, 'actor', 'admin_actor', 'order-manager', 'order-opposite')]
    contention = [competing_management(database, False), competing_management(database, True)]
    detected = mutations(database)
    permission(database, True)
    cleanup = timeout_cleanup(database)
    database.sql("SELECT sp004.change_creation('actor','admin_actor',true,false,'manager-restore');",
                 user='sp004_manager_test')
    if query(database, "SELECT json_agg(a ORDER BY actor_id) FROM kehila.actors a") != original_actors:
        raise ValueError('authority fixtures did not restore actor permissions')
    return {'schedules': results, 'lock_orders': orders, 'management_contention': contention,
            'mutations_detected': detected, 'timeout_cleanup': cleanup, 'actor_state_restored': True,
            'multiline_evidence_regression_detected': True,
            'creation_management_boundaries': boundary_cases,
            'qualified': 'native structural creation/replay and labelled project fixtures only'}
