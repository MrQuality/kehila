-- Private shared grant predicate. The writer calls it in a fresh statement
-- after its project wait; a reader calls it inside its coherent read snapshot.
CREATE FUNCTION kehila_auth.require_item_project_grant(
  requested_actor kehila.ident, requested_project kehila.ident
) RETURNS void LANGUAGE plpgsql STABLE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE grant_state record;
BEGIN
  SELECT g.access_profile,g.active,g.version,
      e.access_profile AS event_profile,e.active AS event_active,e.version AS event_version,
      e.granted_by,e.granting_operation_row_id,e.command_family,e.target_kind,
      p.created_by,p.creation_operation_row_id
    INTO grant_state
    FROM kehila.projects p JOIN kehila.project_grants g USING(project_id)
    LEFT JOIN LATERAL (SELECT event.* FROM kehila.project_grant_events event
      WHERE event.project_id=g.project_id AND event.actor_id=g.actor_id
      ORDER BY event.version DESC LIMIT 1) e ON true
    WHERE p.project_id=requested_project AND g.actor_id=requested_actor;
  IF NOT FOUND OR grant_state.active IS DISTINCT FROM true THEN
    RAISE EXCEPTION 'item project authority denied' USING ERRCODE='42501';
  END IF;
  IF grant_state.access_profile IS DISTINCT FROM 'initial_owner'
     OR grant_state.event_profile IS DISTINCT FROM grant_state.access_profile
     OR grant_state.event_active IS DISTINCT FROM true
     OR grant_state.event_version IS DISTINCT FROM grant_state.version
     OR grant_state.version IS DISTINCT FROM 1::numeric
     OR grant_state.granted_by IS DISTINCT FROM requested_actor
     OR grant_state.command_family IS DISTINCT FROM 'project_create'
     OR grant_state.target_kind IS DISTINCT FROM 'project'
     OR grant_state.created_by IS DISTINCT FROM requested_actor
     OR grant_state.granting_operation_row_id IS DISTINCT FROM grant_state.creation_operation_row_id THEN
    RAISE EXCEPTION 'incoherent current item project grant'
      USING ERRCODE='23514', CONSTRAINT='item_grant_agreement';
  END IF;
END;
$$;

-- Fixed owner-defined write entries invoke this before pending/core/item locks.
CREATE FUNCTION kehila_auth.item_write_fence(
  expected kehila_auth.authority, requested_project kehila.ident
) RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  IF expected.installation_id IS NULL OR expected.owner_actor_id IS NULL
     OR expected.runtime_generation IS NULL OR expected.auth_generation IS NULL
     OR expected.verifier_revision IS NULL THEN
    RAISE EXCEPTION 'complete item authority required' USING ERRCODE='55000';
  END IF;
  fresh := kehila_auth.fence(expected.installation_id,expected.runtime_generation,
    expected.auth_generation,expected.verifier_revision);
  IF fresh.owner_actor_id IS DISTINCT FROM expected.owner_actor_id THEN
    RAISE EXCEPTION 'item authority owner mismatch' USING ERRCODE='42501';
  END IF;
  -- Installation SHARE then actor NO KEY UPDATE are already owned. Never
  -- re-fence after this lock or acquire earlier locks from a later helper.
  PERFORM FROM kehila.projects p WHERE p.project_id=requested_project FOR NO KEY UPDATE;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'item project authority denied' USING ERRCODE='42501';
  END IF;
  -- This new statement starts after the potential project wait.
  PERFORM kehila_auth.require_item_project_grant(fresh.owner_actor_id,requested_project);
  RETURN fresh;
END;
$$;

CREATE FUNCTION kehila_auth.item_read_authority(
  expected kehila_auth.authority, requested_project kehila.ident
) RETURNS kehila_auth.authority LANGUAGE plpgsql STABLE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  IF expected.installation_id IS NULL OR expected.owner_actor_id IS NULL
     OR expected.runtime_generation IS NULL OR expected.auth_generation IS NULL
     OR expected.verifier_revision IS NULL THEN
    RAISE EXCEPTION 'complete item authority required' USING ERRCODE='55000';
  END IF;
  -- Existing helper requires read-only REPEATABLE READ and acquires no fences.
  fresh := kehila_auth.read_authority(expected.installation_id,
    expected.runtime_generation,expected.auth_generation);
  IF fresh.owner_actor_id IS DISTINCT FROM expected.owner_actor_id THEN
    RAISE EXCEPTION 'item authority owner mismatch' USING ERRCODE='42501';
  END IF;
  IF fresh.verifier_revision IS DISTINCT FROM expected.verifier_revision THEN
    RAISE EXCEPTION 'stale item verifier revision' USING ERRCODE='55000';
  END IF;
  PERFORM kehila_auth.require_item_project_grant(fresh.owner_actor_id,requested_project);
  -- No can_create_project or mutable archive/version eligibility substitution.
  -- The application still owns final live-session response eligibility.
  RETURN fresh;
END;
$$;
REVOKE ALL ON FUNCTION kehila_auth.require_item_project_grant(kehila.ident,kehila.ident),
  kehila_auth.item_write_fence(kehila_auth.authority,kehila.ident),
  kehila_auth.item_read_authority(kehila_auth.authority,kehila.ident) FROM PUBLIC;
