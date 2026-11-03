-- Product authentication schema; included inside the fresh product transaction.
SET LOCAL ROLE kehila_owner;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '15s';

CREATE FUNCTION kehila_auth.credentials(requested_installation kehila.ident)
RETURNS TABLE (installation_id kehila.ident, owner_actor_id kehila.ident,
  runtime_generation kehila.u64, auth_generation kehila.u64, verifier_revision kehila.u64,
  policy_version integer, verifier kehila_auth.verifier, owner_active boolean)
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  -- Early, non-authorizing snapshot for bounded password work. A serving
  -- mutation must subsequently fence and compare the verified metadata.
  RETURN QUERY SELECT i.installation_id, i.owner_actor_id, i.runtime_generation,
      c.auth_generation, c.verifier_revision, c.policy_version, c.verifier, a.active
    FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING (owner_actor_id)
    JOIN kehila.actors a ON a.actor_id = i.owner_actor_id
    WHERE i.singleton AND i.installation_id = requested_installation;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'credentials unavailable' USING ERRCODE = '55000';
  END IF;
END;
$$;

-- Owner-only helper: public entry points first acquire their compatible
-- installation/actor fences and supply fresh metadata. No caller chooses an
-- arbitrary actor, event kind or privilege mode through a granted function.
CREATE FUNCTION kehila_auth.apply_credential_change(
  current_state kehila_auth.authority, replacement kehila_auth.verifier,
  event_id kehila_auth.public_id, action text, initiating_session kehila_auth.public_id
) RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE
  changed kehila_auth.authority := current_state;
BEGIN
  IF action IS NULL OR action NOT IN ('recovery','password_change','global_revoke') THEN
    RAISE EXCEPTION 'invalid management action' USING ERRCODE = '22023';
  END IF;
  IF event_id IS NULL OR (action <> 'global_revoke' AND replacement IS NULL)
     OR (action = 'recovery') <> (initiating_session IS NULL) THEN
    RAISE EXCEPTION 'management inputs required' USING ERRCODE = '23502';
  END IF;
  UPDATE kehila_auth.owner_credentials c
    SET auth_generation = c.auth_generation + 1,
        verifier_revision = c.verifier_revision + CASE WHEN action = 'global_revoke' THEN 0 ELSE 1 END,
        verifier = CASE WHEN action = 'global_revoke' THEN c.verifier ELSE replacement END
    WHERE c.owner_actor_id = current_state.owner_actor_id
      AND c.auth_generation = current_state.auth_generation
      AND c.verifier_revision = current_state.verifier_revision
    RETURNING c.auth_generation, c.verifier_revision
      INTO changed.auth_generation, changed.verifier_revision;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'stale credential proof' USING ERRCODE = '55000';
  END IF;
  INSERT INTO kehila_auth.management_events(runtime_generation, public_id, event_kind,
    installation_id, owner_actor_id, auth_generation, verifier_revision,
    initiator_kind, initiating_session_id)
    VALUES (changed.runtime_generation, event_id, action, changed.installation_id,
            changed.owner_actor_id, changed.auth_generation, changed.verifier_revision,
            CASE WHEN action = 'recovery' THEN 'local_admin' ELSE 'owner_session' END,
            initiating_session);
  RETURN changed;
END;
$$;

CREATE FUNCTION kehila_auth.recover(requested_installation kehila.ident,
                               replacement kehila_auth.verifier, event_id kehila_auth.public_id)
RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE
  fresh kehila_auth.authority;
BEGIN
  PERFORM kehila_auth.lock_owner(requested_installation, false);
  SELECT i.installation_id, i.owner_actor_id, i.runtime_generation,
    c.auth_generation, c.verifier_revision INTO STRICT fresh
    FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING (owner_actor_id)
    WHERE i.singleton;
  -- Intentionally independent of actor activity/grants. Administrative recovery
  -- replaces credentials, not identity, permissions, history or runtime generation.
  RETURN kehila_auth.apply_credential_change(fresh, replacement, event_id, 'recovery', NULL);
END;
$$;

CREATE FUNCTION kehila_auth.change_password(
  requested_installation kehila.ident, expected_runtime kehila.u64,
  expected_auth kehila.u64, expected_revision kehila.u64,
  replacement kehila_auth.verifier, event_id kehila_auth.public_id, initiating_session kehila_auth.public_id
) RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE
  fresh kehila_auth.authority;
BEGIN
  IF expected_revision IS NULL THEN
    RAISE EXCEPTION 'verified revision required' USING ERRCODE = '23502';
  END IF;
  fresh := kehila_auth.fence(requested_installation, expected_runtime, expected_auth, expected_revision);
  RETURN kehila_auth.apply_credential_change(fresh, replacement, event_id, 'password_change', initiating_session);
END;
$$;

CREATE FUNCTION kehila_auth.revoke_all(
  requested_installation kehila.ident, expected_runtime kehila.u64,
  expected_auth kehila.u64, expected_revision kehila.u64,
  event_id kehila_auth.public_id, initiating_session kehila_auth.public_id
) RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE
  fresh kehila_auth.authority;
BEGIN
  IF expected_revision IS NULL THEN
    RAISE EXCEPTION 'verified revision required' USING ERRCODE = '23502';
  END IF;
  fresh := kehila_auth.fence(requested_installation, expected_runtime, expected_auth, expected_revision);
  RETURN kehila_auth.apply_credential_change(fresh, NULL, event_id, 'global_revoke', initiating_session);
END;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA kehila_auth FROM PUBLIC;
GRANT USAGE ON TYPE kehila_auth.authority TO kehila_auth_admin;
GRANT EXECUTE ON FUNCTION kehila_auth.credentials(kehila.ident),
  kehila_auth.change_password(kehila.ident, kehila.u64, kehila.u64, kehila.u64,
                         kehila_auth.verifier, kehila_auth.public_id, kehila_auth.public_id),
  kehila_auth.revoke_all(kehila.ident, kehila.u64, kehila.u64, kehila.u64,
                    kehila_auth.public_id, kehila_auth.public_id) TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION kehila_auth.recover(kehila.ident, kehila_auth.verifier, kehila_auth.public_id)
  TO kehila_auth_admin;
