-- Independent fresh-fixture oracle. Run only after the exact native journey.
SELECT json_build_object(
  'projects', (SELECT count(*) FROM kehila.projects),
  'operations', (SELECT count(*) FROM kehila.operations),
  'payloads', (SELECT count(*) FROM kehila.operation_payloads),
  'mappings', (SELECT count(*) FROM kehila_auth.creation_intents),
  'revisions', (SELECT count(*) FROM kehila.configuration_revisions),
  'grants', (SELECT count(*) FROM kehila.project_grants),
  'grant_events', (SELECT count(*) FROM kehila.project_grant_events),
  'pending', (SELECT count(*) FROM kehila_auth.pending_projects),
  'completed', (SELECT count(*) FROM kehila_auth.pending_projects WHERE state = 'completed'),
  'cancelled', (SELECT count(*) FROM kehila_auth.pending_projects WHERE state = 'cancelled'),
  'prepared', (SELECT count(*) FROM kehila_auth.pending_projects WHERE state = 'prepared'),
  'expired', (SELECT count(*) FROM kehila_auth.pending_projects WHERE state = 'expired'),
  'runtime_connections', (SELECT count(*) FROM pg_stat_activity WHERE usename = 'kehila_auth_runtime_login'),
  'authority_locks', (SELECT count(*) FROM pg_locks WHERE locktype = 'advisory' AND classid = 1262831689::oid));
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila.projects) <> 3
    OR (SELECT count(*) FROM kehila.operations) <> 3
    OR (SELECT count(*) FROM kehila.operation_payloads) <> 3
    OR (SELECT count(*) FROM kehila_auth.creation_intents) <> 3
    OR (SELECT count(*) FROM kehila.configuration_revisions) <> 3
    OR (SELECT count(*) FROM kehila.project_grants) <> 3
    OR (SELECT count(*) FROM kehila.project_grant_events) <> 3 THEN
    RAISE EXCEPTION 'native success cardinality mismatch';
  END IF;
  -- Later registration prunes the earlier cancelled entry. The final
  -- acknowledgement-loss case leaves one cancelled entry after the three
  -- completed requests, with no further registration to prune it.
  IF (SELECT count(*) FROM kehila_auth.pending_projects) <> 4
    OR (SELECT count(*) FROM kehila_auth.pending_projects WHERE state = 'completed') <> 3
    OR (SELECT count(*) FROM kehila_auth.pending_projects WHERE state = 'cancelled') <> 1
    OR EXISTS (SELECT FROM kehila_auth.pending_projects WHERE state NOT IN ('completed','cancelled')) THEN
    RAISE EXCEPTION 'native pending cardinality mismatch';
  END IF;
  IF EXISTS (SELECT FROM kehila.projects p LEFT JOIN kehila_auth.creation_intents i
    ON i.operation_row_id = p.creation_operation_row_id AND i.target_project_id = p.project_id
    WHERE i.request_id IS NULL) THEN
    RAISE EXCEPTION 'native project identity mismatch';
  END IF;
  IF EXISTS (SELECT FROM pg_stat_activity WHERE usename = 'kehila_auth_runtime_login')
    OR EXISTS (SELECT FROM pg_locks WHERE locktype = 'advisory' AND classid = 1262831689::oid) THEN
    RAISE EXCEPTION 'native runtime ownership remains';
  END IF;
END;
$$;
