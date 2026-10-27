"""SQL audit confirmation only; real credential eligibility belongs to Rust."""
from check_postgresql import preflight
from sessions import Session, blocked
from enrollment_cases import denied, require
from authority_cases import RUNTIME
from credential_cases import start

ISSUE = "SELECT auth001.audit_issue('installation',9,5,4,'browser-session','browser');"
LOGOUT = "SELECT auth001.audit_logout('installation',9,5,'browser-session','browser');"
REVOKE = "SELECT auth001.audit_revoke('installation',9,5,4,'cli-session','cli','browser-session');"


def snapshot(db):
    return db.sql("SELECT coalesce(jsonb_agg(to_jsonb(e) ORDER BY runtime_generation,public_id,event_kind),"
                  "'[]'::jsonb)::text FROM auth001.management_events e "
                  "WHERE event_kind IN ('session_issue','logout','session_revoke');")


def confirmation(db):
    with Session(db, RUNTIME) as lock:
        start(lock, 'audit-confirmation', 'installation|owner|9|5|4')
        denied(db, "SELECT auth001.audit_issue('installation',9,5,NULL,'no-proof','browser');",
               '23502', user=RUNTIME)
        denied(db, "SELECT auth001.audit_issue('installation',9,5,99,'stale-proof','browser');",
               '55000', user=RUNTIME)
        db.sql(ISSUE, user=RUNTIME)  # Simulate losing a committed response.
        before = snapshot(db)
        require(db.sql(ISSUE, user=RUNTIME) == 't', 'issuance audit retry not confirmed')
        require(snapshot(db) == before, 'issuance retry changed event/timestamp')
        denied(db, ISSUE.replace("'browser');", "'cli');"), 'XX001', user=RUNTIME)
        require(snapshot(db) == before, 'conflicting issuance changed original event')
        require(db.sql("SELECT auth001.audit_issue('installation',9,5,4,'cli-session','cli');",
                       user=RUNTIME) == 't', 'CLI issuance audit failed')
        with Session(db, RUNTIME) as first, Session(db, RUNTIME) as second, \
                Session(db, 'postgres') as observer:
            first.execute('BEGIN;')
            require(first.execute(LOGOUT) == 't', 'first logout audit not confirmed')
            second.send(LOGOUT)
            blocked(observer, second, first)
            first.execute('COMMIT;')
            require(second.wait() == 't', 'concurrent identical logout not confirmed')
        before = snapshot(db)
        require(db.sql(LOGOUT, user=RUNTIME) == 't', 'logout retry not confirmed')
        require(snapshot(db) == before, 'logout retry changed event/timestamp')
        require(db.sql(REVOKE, user=RUNTIME) == 't', 'individual revocation not confirmed')
        before = snapshot(db)
        require(db.sql(REVOKE, user=RUNTIME) == 't', 'individual revocation retry not confirmed')
        denied(db, REVOKE.replace("'browser-session');", "'another-session');"), 'XX001', user=RUNTIME)
        require(snapshot(db) == before, 'revocation retry changed original provenance/timestamp')
        for statement in (ISSUE, LOGOUT, REVOKE):
            denied(db, statement, '42501')
            denied(db, statement, '42501', user='auth001_test_unprivileged')
        denied(db, "SELECT auth001.append_session_audit(ROW('installation','owner',9,5,4)"
                   "::auth001.authority,'target','session_issue','cli',NULL);", '42501', user=RUNTIME)
    require(db.sql("SELECT count(*) FROM auth001.management_events WHERE runtime_generation=9 "
                   "AND event_kind IN ('session_issue','logout','session_revoke');") == '4',
            'retries duplicated session audit history')


def failures(db):
    with Session(db, RUNTIME) as lock:
        start(lock, 'audit-failures', 'installation|owner|10|5|4')
        issue = "SELECT auth001.audit_issue('installation',10,5,4,'fault-session','cli');"
        db.sql('BEGIN; SET LOCAL ROLE auth001_test_runtime; ' + issue + ' ROLLBACK;')
        before = snapshot(db)
        require(db.sql(issue, user=RUNTIME) == 't', 'issuance after rollback failed')
        require(snapshot(db) != before, 'rolled-back issuance falsely confirmed as existing')
        before = snapshot(db)
        db.sql("""CREATE FUNCTION auth001.reject_session_audit() RETURNS trigger LANGUAGE plpgsql
          SET search_path=pg_catalog AS $$ BEGIN
            RAISE EXCEPTION 'injected session audit rejection' USING ERRCODE='P0A02'; END; $$;
          CREATE TRIGGER reject_session_audit BEFORE INSERT ON auth001.management_events
            FOR EACH ROW WHEN (NEW.event_kind='logout') EXECUTE FUNCTION auth001.reject_session_audit();""")
        logout = "SELECT auth001.audit_logout('installation',10,5,'fault-session','cli');"
        denied(db, logout, 'P0A02', user=RUNTIME)
        require(snapshot(db) == before, 'rejected audit left history')
        db.sql('DROP TRIGGER reject_session_audit ON auth001.management_events; '
               'DROP FUNCTION auth001.reject_session_audit();')
        require(db.sql(logout, user=RUNTIME) == 't', 'audit after injected failure not confirmed')
        before = snapshot(db)
        db.sql("SELECT * FROM auth001.revoke_all('installation',10,5,4,'audit-generation','fault-session');",
               user=RUNTIME)
        denied(db, logout, '55000', user=RUNTIME)
        denied(db, issue, '55000', user=RUNTIME)
        # A fabricated fresh binding for the old public identity must not
        # confirm its different fixed attributes. Real Rust eligibility must
        # deny this rebinding before SQL; the SQL check is defense in depth.
        denied(db, "SELECT auth001.audit_logout('installation',10,6,'fault-session','cli');",
               'XX001', user=RUNTIME)
        require(snapshot(db) == before, 'stale authority changed/confirmed audit history')
    with Session(db, RUNTIME) as replacement:
        start(replacement, 'audit-takeover', 'installation|owner|11|6|4')
        denied(db, "SELECT auth001.audit_logout('installation',10,6,'fault-session','cli');",
               '55000', user=RUNTIME)


def run(db):
    confirmation(db)
    preflight()
    failures(db)
    return ['fixed_identity_session_audit_confirmation', 'session_audit_failure_and_stale_denial']
