-- Confirmation returns no credential and proves no volatile registry eligibility.
BEGIN;
SET LOCAL ROLE kehila_owner;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '15s';

CREATE FUNCTION auth001.append_session_audit(
  current_state auth001.authority, requested_id auth001.public_id,
  action text, expected_kind text, initiating_session auth001.public_id
) RETURNS boolean LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  existing auth001.management_events;
  initiator text := CASE WHEN action = 'session_issue' THEN 'runtime' ELSE 'owner_session' END;
BEGIN
  IF action IS NULL OR action NOT IN ('session_issue','logout','session_revoke') THEN
    RAISE EXCEPTION 'invalid audit action' USING ERRCODE = '22023';
  END IF;
  IF requested_id IS NULL OR (action = 'session_issue') <> (initiating_session IS NULL) THEN
    RAISE EXCEPTION 'audit identity required' USING ERRCODE = '23502';
  END IF;
  IF expected_kind IS NULL OR expected_kind NOT IN ('browser','cli') THEN
    RAISE EXCEPTION 'invalid credential kind' USING ERRCODE = '23514';
  END IF;
  INSERT INTO auth001.management_events(runtime_generation, public_id, event_kind,
    installation_id, owner_actor_id, auth_generation, verifier_revision,
    credential_kind, initiator_kind, initiating_session_id)
    VALUES (current_state.runtime_generation, requested_id, action,
      current_state.installation_id, current_state.owner_actor_id,
      current_state.auth_generation, current_state.verifier_revision,
      expected_kind, initiator, initiating_session)
    ON CONFLICT (runtime_generation, public_id, event_kind) DO NOTHING;
  IF FOUND THEN
    RETURN true;
  END IF;
  SELECT e.* INTO STRICT existing FROM auth001.management_events e
    WHERE e.runtime_generation = current_state.runtime_generation
      AND e.public_id = requested_id AND e.event_kind = action;
  -- Compare every fixed attribute, including nulls; preserve the first timestamp.
  IF ROW(existing.runtime_generation, existing.public_id, existing.event_kind,
         existing.installation_id, existing.owner_actor_id, existing.auth_generation,
         existing.verifier_revision, existing.credential_kind, existing.initiator_kind,
         existing.initiating_session_id)
     IS DISTINCT FROM
     ROW(current_state.runtime_generation, requested_id, action,
         current_state.installation_id, current_state.owner_actor_id, current_state.auth_generation,
         current_state.verifier_revision, expected_kind, initiator, initiating_session) THEN
    RAISE EXCEPTION 'conflicting session audit identity' USING ERRCODE = 'XX001';
  END IF;
  RETURN true;
END;
$$;

CREATE FUNCTION auth001.audit_issue(
  requested_installation kehila.ident, expected_runtime kehila.u64, expected_auth kehila.u64,
  expected_revision kehila.u64, requested_id auth001.public_id, expected_kind text
) RETURNS boolean LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  fresh auth001.authority;
BEGIN
  IF expected_revision IS NULL THEN
    RAISE EXCEPTION 'verified revision required' USING ERRCODE = '23502';
  END IF;
  fresh := auth001.fence(requested_installation, expected_runtime, expected_auth, expected_revision);
  RETURN auth001.append_session_audit(fresh, requested_id, 'session_issue', expected_kind, NULL);
END;
$$;

CREATE FUNCTION auth001.audit_logout(
  requested_installation kehila.ident, expected_runtime kehila.u64, expected_auth kehila.u64,
  requested_id auth001.public_id, expected_kind text
) RETURNS boolean LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  fresh auth001.authority;
BEGIN
  fresh := auth001.fence(requested_installation, expected_runtime, expected_auth);
  RETURN auth001.append_session_audit(fresh, requested_id, 'logout', expected_kind, requested_id);
END;
$$;

CREATE FUNCTION auth001.audit_revoke(
  requested_installation kehila.ident, expected_runtime kehila.u64, expected_auth kehila.u64,
  expected_revision kehila.u64, requested_id auth001.public_id, expected_kind text,
  initiating_session auth001.public_id
) RETURNS boolean LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  fresh auth001.authority;
BEGIN
  IF expected_revision IS NULL THEN
    RAISE EXCEPTION 'verified revision required' USING ERRCODE = '23502';
  END IF;
  fresh := auth001.fence(requested_installation, expected_runtime, expected_auth, expected_revision);
  RETURN auth001.append_session_audit(fresh, requested_id, 'session_revoke', expected_kind, initiating_session);
END;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA auth001 FROM PUBLIC;
GRANT EXECUTE ON FUNCTION
  auth001.audit_issue(kehila.ident, kehila.u64, kehila.u64, kehila.u64, auth001.public_id, text),
  auth001.audit_logout(kehila.ident, kehila.u64, kehila.u64, auth001.public_id, text),
  auth001.audit_revoke(kehila.ident, kehila.u64, kehila.u64, kehila.u64,
                      auth001.public_id, text, auth001.public_id) TO auth001_runtime;
COMMIT;
