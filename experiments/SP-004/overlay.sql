-- Experimental A05 only. Apply after the native verifier in a disposable database.
-- This is not a production migration or an authentication implementation.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL statement_timeout = '10s';
SET LOCAL lock_timeout = '2s';

CREATE ROLE sp004_manager NOLOGIN;
CREATE ROLE sp004_manager_test LOGIN IN ROLE sp004_manager;
CREATE SCHEMA sp004 AUTHORIZATION kehila_owner;
SET LOCAL ROLE kehila_owner;

CREATE TABLE sp004.management_actors (
  actor_id kehila.ident PRIMARY KEY REFERENCES kehila.actors(actor_id)
);
-- Verified identity is a declared fixture, not an end-user credential.
INSERT INTO kehila.actors VALUES
  ('admin_actor', true, false), ('inactive_manager', false, false),
  ('unprivileged_actor', true, false);
INSERT INTO sp004.management_actors VALUES ('actor'), ('admin_actor'), ('inactive_manager');

ALTER TABLE kehila.operations DROP CONSTRAINT slice1_operation_family;
ALTER TABLE kehila.operations DROP CONSTRAINT operations_family_target;
ALTER TABLE kehila.operations ADD CONSTRAINT sp004_operation_family_target CHECK (
  target_kind = 'project' AND command_family IN ('project_create', 'sp004_grant_admin')
);
ALTER TABLE kehila.operations DROP CONSTRAINT slice1_operation_grant;
ALTER TABLE kehila.operations ADD CONSTRAINT sp004_operation_grant CHECK (
  grant_policy_version = 1 AND required_grant = command_family
);
ALTER TABLE kehila.project_grant_events DROP CONSTRAINT slice1_grant_event;
ALTER TABLE kehila.project_grant_events ADD CONSTRAINT sp004_grant_event CHECK (
  access_profile = 'initial_owner' AND (
    (command_family = 'project_create' AND active AND version = 1 AND actor_id = granted_by)
    OR (command_family = 'sp004_grant_admin' AND version >= 2)
  )
);

-- Creator INSERT rights must not admit the newly added management shapes.
CREATE FUNCTION sp004.management_write_only() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, sp004, pg_temp AS $$
BEGIN
  IF NEW.command_family = 'sp004_grant_admin' AND current_user <> 'kehila_owner' THEN
    RAISE EXCEPTION 'management writes require the restricted helper'
      USING ERRCODE = '42501';
  END IF;
  RETURN NEW;
END;
$$;
CREATE TRIGGER sp004_operation_management_only BEFORE INSERT ON kehila.operations
  FOR EACH ROW EXECUTE FUNCTION sp004.management_write_only();
CREATE TRIGGER sp004_event_management_only BEFORE INSERT ON kehila.project_grant_events
  FOR EACH ROW EXECUTE FUNCTION sp004.management_write_only();

CREATE FUNCTION sp004.change_grant(
  manager kehila.ident, subject kehila.ident, project kehila.ident,
  expected_version kehila.u64, new_active boolean, token kehila.operation_token
) RETURNS kehila.u64
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, kehila, sp004, pg_temp AS $$
DECLARE
  current_grant kehila.project_grants%ROWTYPE;
  latest_event kehila.project_grant_events%ROWTYPE;
  next_version kehila.u64;
  operation_id bigint;
  origin_ms kehila.u64;
  request_bytes bytea;
BEGIN
  IF manager IS NULL OR subject IS NULL OR project IS NULL OR expected_version IS NULL
     OR new_active IS NULL OR token IS NULL THEN
    RAISE EXCEPTION 'all management inputs are required' USING ERRCODE = '22004';
  END IF;
  -- Same actor fence as creation; lock the complete set before other rows.
  PERFORM a.actor_id FROM kehila.actors a
    WHERE a.actor_id = ANY(ARRAY[manager, subject])
    ORDER BY a.actor_id COLLATE "C" FOR NO KEY UPDATE;
  IF NOT EXISTS (SELECT FROM kehila.actors a JOIN sp004.management_actors m USING(actor_id)
                 WHERE a.actor_id = manager AND a.active)
     OR NOT EXISTS (SELECT FROM kehila.actors WHERE actor_id = subject) THEN
    RAISE EXCEPTION 'management authority denied' USING ERRCODE = '42501';
  END IF;
  SELECT * INTO current_grant FROM kehila.project_grants
    WHERE project_id = project AND actor_id = subject;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'grant does not exist' USING ERRCODE = '23503';
  END IF;
  SELECT * INTO latest_event FROM kehila.project_grant_events
    WHERE project_id = project AND actor_id = subject ORDER BY version DESC LIMIT 1;
  IF NOT FOUND OR (current_grant.version, current_grant.access_profile, current_grant.active)
      IS DISTINCT FROM (latest_event.version, latest_event.access_profile, latest_event.active)
      OR current_grant.access_profile <> 'initial_owner' THEN
    RAISE EXCEPTION 'current grant and latest history disagree' USING ERRCODE = 'XX001';
  END IF;
  IF current_grant.version <> expected_version THEN
    RAISE EXCEPTION 'stale grant version' USING ERRCODE = '40001';
  END IF;
  IF current_grant.active = new_active THEN
    RAISE EXCEPTION 'grant change is a no-op' USING ERRCODE = '22023';
  END IF;
  next_version := current_grant.version + 1;
  -- Experimental PostgreSQL-16 representation, not project_create_codec v1.
  request_bytes := convert_to(jsonb_build_array(
    'sp004_grant_admin_v1', manager, subject, project, expected_version::text, new_active
  )::text, 'UTF8');
  origin_ms := trunc(extract(epoch FROM clock_timestamp()) * 1000);
  INSERT INTO kehila.operations (
    actor_id, command_family, target_kind, target_project_id, token,
    request_codec_version, request_sha256, required_grant, grant_policy_version,
    replay_origin_ms, replay_deadline_ms
  ) VALUES (manager, 'sp004_grant_admin', 'project', project, token,
    1, sha256(request_bytes), 'sp004_grant_admin', 1, origin_ms, origin_ms + 7776000000)
  RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES (
    operation_id, request_bytes, 1,
    jsonb_build_object('kind', 'sp004_grant_change', 'version', next_version::text,
                      'actor', subject, 'project', project, 'active', new_active)
  );
  UPDATE kehila.project_grants SET version = next_version, active = new_active
    WHERE project_id = project AND actor_id = subject;
  INSERT INTO kehila.project_grant_events (
    project_id, actor_id, version, access_profile, active, granted_by,
    granting_operation_row_id, command_family
  ) VALUES (project, subject, next_version, current_grant.access_profile, new_active,
    manager, operation_id, 'sp004_grant_admin');
  RETURN next_version;
END;
$$;

REVOKE ALL ON SCHEMA sp004 FROM PUBLIC;
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA sp004 FROM PUBLIC;
GRANT USAGE ON SCHEMA sp004 TO kehila_creator, sp004_manager;
GRANT USAGE ON SCHEMA kehila TO sp004_manager;
GRANT USAGE ON TYPE kehila.ident, kehila.u64, kehila.operation_token TO sp004_manager;
-- Deferred payload triggers execute under the committing login, after the
-- SECURITY DEFINER helper returns. They require reads, not direct mutation rights.
GRANT SELECT ON kehila.operations, kehila.operation_payloads TO sp004_manager;
GRANT EXECUTE ON FUNCTION sp004.change_grant(
  kehila.ident, kehila.ident, kehila.ident, kehila.u64, boolean, kehila.operation_token
) TO sp004_manager;
COMMIT;
