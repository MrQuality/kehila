-- Independent observer for one completed, isolated authentication fixture.
DO $$ BEGIN
  IF EXISTS (SELECT FROM kehila.projects)
    OR EXISTS (SELECT FROM kehila.operations)
    OR EXISTS (SELECT FROM kehila.operation_payloads)
    OR EXISTS (SELECT FROM kehila_auth.pending_projects)
    OR EXISTS (SELECT FROM kehila_auth.creation_intents) THEN
    RAISE EXCEPTION 'authentication fixture unexpectedly created domain data';
  END IF;
  IF EXISTS (SELECT FROM pg_stat_activity WHERE datname=current_database()
    AND usename='kehila_auth_runtime_login')
    OR EXISTS (SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) THEN
    RAISE EXCEPTION 'authentication fixture retains runtime ownership';
  END IF;
END $$;
SELECT 'NATIVE_AUTH_FIXTURE_STATE_PASS';
