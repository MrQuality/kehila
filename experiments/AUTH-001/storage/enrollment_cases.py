"""Native A1 enrollment oracles; never run against an existing installation."""
from contextlib import ExitStack

from sessions import Session, SqlFailure, blocked
from check_postgresql import preflight


def require(condition, message):
    if not condition:
        raise ValueError(message)


def denied(db, statement, state, *, user='auth001_test_admin'):
    db.sql(statement, user=user, expected_state=state)


def enrollment(db):
    before = db.sql('SELECT count(*) FROM kehila.actors;')
    for verifier in ("''", "repeat('x',513)", 'chr(233)'):
        denied(db, "SELECT * FROM auth001.enroll('installation','owner','invalid',"
                   + verifier + ');', '23514')
    denied(db, "SELECT * FROM auth001.enroll('installation','owner','invalid',NULL);", '23502')
    db.sql("INSERT INTO kehila.actors VALUES ('preexisting',false,false);")
    denied(db, "SELECT * FROM auth001.enroll('installation','preexisting','invalid',"
               "'synthetic-verifier');", '23505')
    require(db.sql("SELECT NOT active AND NOT can_create_project FROM kehila.actors "
                   "WHERE actor_id='preexisting';") == 't', 'enrollment adopted an existing actor')
    db.sql("DELETE FROM kehila.actors WHERE actor_id='preexisting';")
    require(db.sql('SELECT owner_actor_id IS NULL FROM auth001.installation;') == 't',
            'failed enrollment left owner binding')
    require(db.sql('SELECT count(*) FROM auth001.management_events;') == '0',
            'failed enrollment left audit history')
    # A rolled-back attempt must leave neither an actor nor a binding.
    db.sql("BEGIN; SET LOCAL ROLE auth001_test_admin; "
           "SELECT * FROM auth001.enroll('installation','owner','rolled-back',"
           "'synthetic-verifier'); ROLLBACK;")
    require(db.sql('SELECT count(*) FROM kehila.actors;') == before,
            'rollback leaked an enrolled actor')
    require(db.sql('SELECT owner_actor_id IS NULL FROM auth001.installation;') == 't',
            'rollback retained owner binding')

    with ExitStack() as stack:
        winner, contender, observer = [stack.enter_context(Session(db, user)) for user in
                                       ('auth001_test_admin', 'auth001_test_admin', 'postgres')]
        winner.execute('BEGIN;')
        winner.execute("SELECT * FROM auth001.enroll('installation','owner','enrollment',"
                       "'synthetic-verifier');")
        contender.send("SELECT * FROM auth001.enroll('other-installation','other-owner',"
                       "'other-enrollment','other-verifier');")
        blocked(observer, contender, winner)
        winner.execute('COMMIT;')
        try:
            contender.wait()
        except SqlFailure as error:
            require(error.state == '23505', 'competing enrollment returned wrong SQLSTATE')
        else:
            raise ValueError('competing enrollment replaced the installed owner')

    require(db.sql('SELECT count(*) FROM kehila.actors;') == str(int(before) + 1),
            'competing enrollment left a second actor')
    require(db.sql("SELECT active AND can_create_project FROM kehila.actors "
                   "WHERE actor_id='owner';") == 't', 'initial owner lacks initial authority')
    require(db.sql('SELECT count(*) FROM auth001.management_events;') == '1',
            'enrollment did not have one atomic audit event')
    require(db.sql('SELECT runtime_generation,public_id,event_kind,installation_id,owner_actor_id,'
                   'auth_generation,verifier_revision,initiator_kind,credential_kind IS NULL,'
                   'initiating_session_id IS NULL FROM auth001.management_events;')
            == '0|enrollment|enrollment|installation|owner|1|1|local_admin|t|t',
            'enrollment audit attributes do not match the installed claim')
    # Simulate losing the successful response: reconcile by fixed claim identity.
    metadata = db.sql("SELECT * FROM auth001.enrollment_status('installation','owner',"
                      "'enrollment');", user='auth001_test_admin')
    require(metadata == 'installation|owner|0|1|1', 'enrollment reconciliation metadata mismatch')
    denied(db, "SELECT * FROM auth001.enrollment_status('installation','owner','wrong');", '55000')
    denied(db, "SELECT * FROM auth001.enroll('installation','owner','enrollment',"
               "'replacement-verifier');", '23505')
    require(db.sql('SELECT verifier FROM auth001.owner_credentials;') == 'synthetic-verifier',
            'retry silently replaced the password verifier')


def capabilities(db):
    for user in ('auth001_test_runtime', 'auth001_test_admin'):
        for statement in (
            'SELECT * FROM auth001.owner_credentials;',
            'DELETE FROM auth001.installation;',
            "UPDATE kehila.actors SET active=true;",
            "INSERT INTO auth001.management_events DEFAULT VALUES;",
            'TRUNCATE auth001.management_events;',
        ):
            denied(db, statement, '42501', user=user)
    denied(db, "SELECT * FROM auth001.enrollment_status('installation','owner','enrollment');",
           '42501', user='auth001_test_runtime')
    denied(db, "SELECT * FROM auth001.enroll('x','y','z','synthetic-verifier');", '42501',
           user='auth001_test_runtime')
    require(db.sql("SELECT count(*) FROM pg_auth_members WHERE member = "
                   "'auth001_runtime'::regrole AND roleid IN "
                   "('auth001_admin'::regrole,'kehila_creator'::regrole,'kehila_owner'::regrole);") == '0',
            'runtime inherited administrative or business-write authority')


def constraints(db):
    # Object-owner attempts exercise storage invariants beyond serving privileges.
    denied(db, "UPDATE auth001.installation SET owner_actor_id=NULL;", '23514', user='postgres')
    denied(db, "UPDATE auth001.installation SET enrollment_attempt_id='different';", '23514',
           user='postgres')
    denied(db, 'DELETE FROM auth001.installation;', '23514', user='postgres')
    denied(db, 'TRUNCATE auth001.installation CASCADE;', '23514', user='postgres')
    denied(db, 'UPDATE auth001.management_events SET recorded_at=clock_timestamp();', '23514',
           user='postgres')
    denied(db, 'DELETE FROM auth001.management_events;', '23514', user='postgres')
    denied(db, 'TRUNCATE auth001.management_events;', '23514', user='postgres')
    denied(db, "UPDATE auth001.owner_credentials SET verifier=repeat('x',513);", '23514',
           user='postgres')
    denied(db, "UPDATE auth001.owner_credentials SET verifier=chr(233);", '23514', user='postgres')
    denied(db, 'UPDATE auth001.owner_credentials SET auth_generation=0;', '23514', user='postgres')
    denied(db, 'UPDATE auth001.owner_credentials SET verifier_revision=18446744073709551616;',
           '23514', user='postgres')
    # Blocked is represented explicitly; it is not an empty-password verifier.
    db.sql('BEGIN; UPDATE auth001.owner_credentials SET verifier=NULL; ROLLBACK;')


def run(db):
    enrollment(db)
    preflight()
    capabilities(db)
    preflight()
    constraints(db)
    return ['atomic_competing_enrollment', 'simulated_lost_response_reconciliation',
            'restricted_capabilities', 'binding_verifier_and_audit_constraints']
