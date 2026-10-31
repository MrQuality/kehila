"""Native credential-generation races and actor-preserving admin recovery."""
from contextlib import ExitStack

from check_postgresql import preflight
from sessions import Session, SqlFailure, blocked
from enrollment_cases import denied, require
from authority_cases import RUNTIME, acquire, reject_waiter


def start(session, attempt, expected):
    acquire(session)
    require(session.execute("SELECT * FROM auth001.start_runtime('installation','" + attempt + "');")
            == expected, 'credential fixture runtime metadata mismatch')


def credentials(db):
    return db.sql("SELECT * FROM auth001.credentials('installation');", user=RUNTIME)


def races(db):
    with ExitStack() as stack:
        lock, admin, serving, observer = [stack.enter_context(Session(db, user)) for user in
                                         (RUNTIME, 'auth001_test_admin', RUNTIME, 'postgres')]
        start(lock, 'credential-races', 'installation|owner|5|1|1')
        require(credentials(db) == 'installation|owner|5|1|1|1|synthetic-verifier|t',
                'early credential snapshot mismatch')
        admin.execute('BEGIN;')
        admin.execute("SELECT * FROM auth001.recover('installation','recovered-verifier','recover-wait');")
        serving.send("SELECT * FROM auth001.fence('installation',5,1,1);")
        blocked(observer, serving, admin)
        admin.execute('COMMIT;')
        reject_waiter(serving)
    # New participants keep each 60-second ownership budget independent.
    with Session(db, RUNTIME) as lock, Session(db, RUNTIME) as first, \
            Session(db, RUNTIME) as second, Session(db, 'postgres') as observer:
        start(lock, 'password-races', 'installation|owner|6|2|2')
        first.execute('BEGIN;')
        require(first.execute("SELECT * FROM auth001.change_password('installation',6,2,2,"
                              "'changed-verifier','password-change','initiator');")
                == 'installation|owner|6|3|3', 'password replacement metadata mismatch')
        second.send("SELECT * FROM auth001.change_password('installation',6,2,2,"
                    "'losing-verifier','losing-change','other-initiator');")
        blocked(observer, second, first)
        first.execute('COMMIT;')
        reject_waiter(second)
        require(db.sql("SELECT * FROM auth001.revoke_all('installation',6,3,3,'global','initiator');",
                       user=RUNTIME) == 'installation|owner|6|4|3',
                'generation-wide revocation changed verifier revision')
        require(credentials(db) == 'installation|owner|6|4|3|1|changed-verifier|t',
                'losing password proof replaced the verifier')
        denied(db, "SELECT * FROM auth001.change_password('installation',6,3,3,"
                   "'old-proof','after-global','initiator');", '55000', user=RUNTIME)
        require(db.sql("SELECT event_kind,auth_generation,verifier_revision,initiator_kind,"
                       "initiating_session_id FROM auth001.management_events "
                       "WHERE event_kind IN ('password_change','global_revoke') ORDER BY event_kind;")
                == 'global_revoke|4|3|owner_session|initiator\npassword_change|3|3|owner_session|initiator',
                'credential management audit metadata mismatch')


def history_fixture(db):
    # Structural foundation rows only, not a domain command/codec qualification.
    db.sql("""BEGIN; DO $$ DECLARE op bigint; BEGIN
      INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_project_id,
        token,request_codec_version,request_sha256,required_grant,grant_policy_version,
        replay_origin_ms,replay_deadline_ms)
      VALUES ('owner','project_create','project','history-project','history-token',1,
        decode(repeat('00',32),'hex'),'project_create',1,0,7776000000)
      RETURNING operation_row_id INTO op;
      INSERT INTO kehila.operation_payloads VALUES (op,decode('01','hex'),1,'{}');
      INSERT INTO kehila.projects(project_id,name,current_prefix,estimate_unit,
        configuration_revision,next_sequence,created_by,creation_operation_row_id,seed_profile)
      VALUES ('history-project','Structural recovery fixture','HF','hours',1,1,'owner',op,'m1_v1');
      INSERT INTO kehila.configuration_revisions VALUES
        ('history-project',1,1,'{}','{}','owner',op,'project_create','project');
      INSERT INTO kehila.project_grants VALUES ('history-project','owner','initial_owner',true,1);
      INSERT INTO kehila.project_grant_events(project_id,actor_id,version,access_profile,
        active,granted_by,granting_operation_row_id,command_family)
      VALUES ('history-project','owner',1,'initial_owner',true,'owner',op,'project_create');
    END; $$; COMMIT;""")


def business_snapshot(db):
    # Fixed allowlist; no externally supplied SQL identifier or unbounded dataset.
    tables = ('actors', 'operations', 'operation_payloads', 'projects',
              'configuration_revisions', 'project_grants', 'project_grant_events')
    return [db.sql('SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY to_jsonb(t)::text),'
                   "'[]'::jsonb)::text FROM kehila." + table + ' t;') for table in tables]


def preserves_owner(db):
    history_fixture(db)
    with Session(db, RUNTIME) as lock:
        start(lock, 'recovery-preserves', 'installation|owner|7|4|3')
        db.sql("UPDATE kehila.actors SET active=false,can_create_project=false WHERE actor_id='owner';")
        before = business_snapshot(db)
        require(db.sql("SELECT * FROM auth001.recover('installation','inactive-recovery','preserve');",
                       user='auth001_test_admin') == 'installation|owner|7|5|4',
                'recovery changed runtime generation or failed to advance credentials')
        require(business_snapshot(db) == before, 'recovery changed actor/grants/business history')
        denied(db, "SELECT * FROM auth001.recover('installation','replacement','preserve');", '23505')
        require(credentials(db) == 'installation|owner|7|5|4|1|inactive-recovery|f',
                'duplicate recovery replaced verifier or reactivated owner')
        require(db.sql("SELECT auth_generation,verifier_revision,initiator_kind,"
                       "initiating_session_id IS NULL FROM auth001.management_events "
                       "WHERE runtime_generation=7 AND public_id='preserve' AND event_kind='recovery';")
                == '5|4|local_admin|t', 'administrative recovery audit metadata mismatch')
        denied(db, "SELECT * FROM auth001.recover('installation','forbidden','runtime-recovery');",
               '42501', user=RUNTIME)
        denied(db, "SELECT * FROM auth001.change_password('installation',7,5,4,"
                   "'forbidden','admin-change','initiator');", '42501')
        denied(db, "SELECT * FROM auth001.credentials('installation');", '42501')
        denied(db, "SELECT auth001.lock_owner('installation',true);", '42501', user=RUNTIME)
        denied(db, "SELECT auth001.apply_credential_change(ROW('installation','owner',7,5,4)"
                   "::auth001.authority,'forbidden','private','recovery',NULL);", '42501', user=RUNTIME)
        db.sql("UPDATE kehila.actors SET active=true,can_create_project=true WHERE actor_id='owner';")


def rollback(db):
    with Session(db, RUNTIME) as lock:
        start(lock, 'credential-rollback', 'installation|owner|8|5|4')
        before = credentials(db)
        events = db.sql('SELECT count(*) FROM auth001.management_events;')
        for column in ('auth_generation', 'verifier_revision'):
            with Session(db, 'postgres') as fixture:
                fixture.execute('BEGIN; UPDATE auth001.owner_credentials SET ' + column
                                + '=18446744073709551615; SET LOCAL ROLE auth001_test_admin;')
                try:
                    fixture.execute("SELECT * FROM auth001.recover('installation','overflow','overflow');")
                except SqlFailure as error:
                    require(error.state == '23514', 'credential overflow returned wrong SQLSTATE')
                else:
                    raise ValueError('credential generation overflow accepted')
            require(credentials(db) == before, 'overflow changed credential state')
        db.sql("""CREATE FUNCTION auth001.reject_audit() RETURNS trigger LANGUAGE plpgsql
          SET search_path=pg_catalog AS $$ BEGIN
            RAISE EXCEPTION 'injected audit rejection' USING ERRCODE='P0A01'; END; $$;
          CREATE TRIGGER reject_audit BEFORE INSERT ON auth001.management_events
            FOR EACH ROW WHEN (NEW.event_kind='password_change') EXECUTE FUNCTION auth001.reject_audit();""")
        denied(db, "SELECT * FROM auth001.change_password('installation',8,5,4,"
                   "'must-rollback','audit-failure','initiator');", 'P0A01', user=RUNTIME)
        db.sql('DROP TRIGGER reject_audit ON auth001.management_events; DROP FUNCTION auth001.reject_audit();')
        require(credentials(db) == before, 'failed audit committed credential replacement')
        require(db.sql('SELECT count(*) FROM auth001.management_events;') == events,
                'failed audit/overflow left management history')


def run(db):
    races(db)
    preflight()
    preserves_owner(db)
    preflight()
    rollback(db)
    return ['post_wait_credential_cas', 'actor_preserving_administrative_recovery',
            'credential_overflow_and_audit_rollback']
