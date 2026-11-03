-- Independent restored-database oracle, after its fresh authority has joined.
SELECT json_build_object(
  'projects',(SELECT count(*) FROM kehila.projects),
  'cores',(SELECT count(*) FROM kehila.operations),
  'payloads',(SELECT count(*) FROM kehila.operation_payloads),
  'mappings',(SELECT count(*) FROM kehila_auth.creation_intents),
  'revisions',(SELECT count(*) FROM kehila.configuration_revisions),
  'grants',(SELECT count(*) FROM kehila.project_grants),
  'grant_events',(SELECT count(*) FROM kehila.project_grant_events),
  'pending',(SELECT count(*) FROM kehila_auth.pending_projects),
  'completed',(SELECT count(*) FROM kehila_auth.pending_projects WHERE state='completed'),
  'cancelled',(SELECT count(*) FROM kehila_auth.pending_projects WHERE state='cancelled'),
  'retired',(SELECT count(*) FROM kehila.operations WHERE payload_retired),
  'runtime_connections',(SELECT count(*) FROM pg_stat_activity WHERE usename='kehila_auth_runtime_login'),
  'authority_locks',(SELECT count(*) FROM pg_locks WHERE locktype='advisory' AND classid=1262831689::oid));
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects) <> 3
    OR (SELECT count(*) FROM kehila.operations) <> 3
    OR (SELECT count(*) FROM kehila.operation_payloads) <> 2
    OR (SELECT count(*) FROM kehila_auth.creation_intents) <> 3
    OR (SELECT count(*) FROM kehila.configuration_revisions) <> 3
    OR (SELECT count(*) FROM kehila.project_grants) <> 3
    OR (SELECT count(*) FROM kehila.project_grant_events) <> 3
    OR (SELECT count(*) FROM kehila.operations WHERE payload_retired) <> 1
    OR (SELECT count(*) FROM kehila_auth.pending_projects) <> 4
    OR (SELECT count(*) FROM kehila_auth.pending_projects WHERE state='completed') <> 3
    OR (SELECT count(*) FROM kehila_auth.pending_projects WHERE state='cancelled') <> 1
    OR EXISTS (SELECT FROM kehila_auth.pending_projects WHERE state NOT IN ('completed','cancelled')) THEN
    RAISE EXCEPTION 'restored native durable totals differ';
  END IF;
  IF EXISTS (SELECT FROM kehila.projects p LEFT JOIN kehila_auth.creation_intents i
    ON i.operation_row_id=p.creation_operation_row_id AND i.target_project_id=p.project_id WHERE i.request_id IS NULL)
    OR EXISTS (SELECT FROM kehila_auth.pending_projects p JOIN kehila.operations o USING(operation_row_id)
      WHERE (p.name IS NULL) IS DISTINCT FROM o.payload_retired)
    OR EXISTS (SELECT FROM kehila.operations o LEFT JOIN kehila.operation_payloads p USING(operation_row_id)
      WHERE o.payload_retired IS DISTINCT FROM (p.operation_row_id IS NULL)) THEN
    RAISE EXCEPTION 'restored native reference/retirement coherence differs';
  END IF;
  IF EXISTS (SELECT FROM pg_stat_activity WHERE usename='kehila_auth_runtime_login')
    OR EXISTS (SELECT FROM pg_locks WHERE locktype='advisory' AND classid=1262831689::oid)
    OR EXISTS (SELECT FROM pg_proc WHERE oid='kehila_auth.trusted_millis()'::regprocedure AND prosecdef) THEN
    RAISE EXCEPTION 'restored native cleanup differs';
  END IF;
END $$;
