-- Explicit experimental audit: never called from serving admission.
-- Apply after the serial SP-004 overlay. No production schema is changed.
BEGIN;
SET LOCAL ROLE kehila_owner;
CREATE FUNCTION sp004.audit_grant(project kehila.ident, subject kehila.ident)
RETURNS bigint LANGUAGE plpgsql SET search_path = pg_catalog, kehila, sp004, pg_temp AS $$
DECLARE e record; previous_version numeric := 0; total bigint := 0;
BEGIN
  FOR e IN
    SELECT h.*, o.actor_id AS manager, o.target_project_id, o.required_grant,
           o.grant_policy_version, o.request_codec_version, o.request_sha256,
           o.command_family AS operation_family,
           o.payload_retired, p.request_bytes, p.result_codec_version, p.result
    FROM kehila.project_grant_events h
    LEFT JOIN kehila.operations o ON o.operation_row_id = h.granting_operation_row_id
    LEFT JOIN kehila.operation_payloads p USING(operation_row_id)
    WHERE h.project_id = project AND h.actor_id = subject ORDER BY h.version
  LOOP
    IF e.version <> previous_version + 1 THEN
      RAISE EXCEPTION 'audit: grant history gap' USING ERRCODE = 'XX001';
    END IF;
    IF e.manager IS DISTINCT FROM e.granted_by OR e.target_project_id IS DISTINCT FROM project
       OR e.access_profile <> 'initial_owner' OR e.grant_policy_version IS DISTINCT FROM 1
       OR e.required_grant IS DISTINCT FROM e.command_family
       OR e.operation_family IS DISTINCT FROM e.command_family THEN
      RAISE EXCEPTION 'audit: grant attribution mismatch' USING ERRCODE = 'XX001';
    END IF;
    IF e.version = 1 THEN
      IF e.command_family <> 'project_create' OR e.actor_id <> e.granted_by OR NOT e.active THEN
        RAISE EXCEPTION 'audit: invalid grant origin' USING ERRCODE = 'XX001';
      END IF;
      -- Exact frozen structural fixture, deliberately not production codec-v1 bytes.
      IF e.request_codec_version IS DISTINCT FROM 1 OR e.result_codec_version IS DISTINCT FROM 1
         OR e.payload_retired IS DISTINCT FROM false OR e.request_bytes IS DISTINCT FROM decode('01','hex')
         OR e.request_sha256 IS DISTINCT FROM decode(repeat('01',32),'hex')
         OR e.result IS DISTINCT FROM '{}'::jsonb THEN
        RAISE EXCEPTION 'audit: creation fixture mismatch' USING ERRCODE = 'XX001';
      END IF;
    ELSE
      IF e.command_family <> 'sp004_grant_admin' OR e.payload_retired IS DISTINCT FROM false
         OR e.request_codec_version IS DISTINCT FROM 1 OR e.result_codec_version IS DISTINCT FROM 1
         OR e.request_sha256 IS DISTINCT FROM sha256(e.request_bytes)
         OR e.request_bytes IS DISTINCT FROM convert_to(jsonb_build_array(
              'sp004_grant_admin_v1', e.granted_by, subject, project,
              (e.version - 1)::text, e.active)::text, 'UTF8')
         OR e.result IS DISTINCT FROM jsonb_build_object(
              'kind', 'sp004_grant_change', 'version', e.version::text,
              'actor', subject, 'project', project, 'active', e.active) THEN
        RAISE EXCEPTION 'audit: grant payload mismatch' USING ERRCODE = 'XX001';
      END IF;
    END IF;
    previous_version := e.version;
    total := total + 1;
  END LOOP;
  IF total = 0 OR NOT EXISTS (
    SELECT FROM kehila.project_grants g JOIN kehila.project_grant_events h
      USING(project_id, actor_id, version)
    WHERE g.project_id = project AND g.actor_id = subject AND g.version = previous_version
      AND (g.access_profile, g.active) IS NOT DISTINCT FROM (h.access_profile, h.active)
  ) THEN
    RAISE EXCEPTION 'audit: current grant mismatch' USING ERRCODE = 'XX001';
  END IF;
  RETURN total;
END;
$$;
REVOKE ALL ON FUNCTION sp004.audit_grant(kehila.ident, kehila.ident) FROM PUBLIC;
COMMIT;
