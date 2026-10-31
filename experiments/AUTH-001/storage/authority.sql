-- Disposable runtime authority only; apply after the enrollment overlay.
BEGIN;
SET LOCAL ROLE kehila_owner;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '15s';

CREATE TYPE auth001.authority AS (
  installation_id kehila.ident,
  owner_actor_id kehila.ident,
  runtime_generation kehila.u64,
  auth_generation kehila.u64,
  verifier_revision kehila.u64
);

-- Private lock primitive. Its boolean is not a serving API or arbitrary actor
-- selector. Callers choose one lock mode before any actor lock; never upgrade.
CREATE FUNCTION auth001.lock_owner(requested_installation kehila.ident, takeover boolean)
RETURNS void LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  installed_owner kehila.ident;
BEGIN
  IF current_setting('transaction_isolation') <> 'read committed' THEN
    RAISE EXCEPTION 'requires READ COMMITTED' USING ERRCODE = '25000';
  END IF;
  IF takeover THEN
    SELECT i.owner_actor_id INTO STRICT installed_owner FROM auth001.installation i
      WHERE i.singleton AND i.installation_id = requested_installation FOR UPDATE;
  ELSE
    SELECT i.owner_actor_id INTO STRICT installed_owner FROM auth001.installation i
      WHERE i.singleton AND i.installation_id = requested_installation FOR SHARE;
  END IF;
  PERFORM FROM kehila.actors a WHERE a.actor_id = installed_owner FOR NO KEY UPDATE;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'installed owner unavailable' USING ERRCODE = '55000';
  END IF;
EXCEPTION WHEN NO_DATA_FOUND THEN
  RAISE EXCEPTION 'installation unavailable' USING ERRCODE = '55000';
END;
$$;

CREATE FUNCTION auth001.fence(
  requested_installation kehila.ident, expected_runtime kehila.u64,
  expected_auth kehila.u64, expected_revision kehila.u64 DEFAULT NULL
) RETURNS auth001.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  fresh record;
BEGIN
  PERFORM auth001.lock_owner(requested_installation, false);
  -- This statement starts AFTER both potential lock waits. Never replace it
  -- with a credential/authority snapshot captured before lock_owner returned.
  SELECT i.installation_id, i.owner_actor_id, i.runtime_generation,
    c.auth_generation, c.verifier_revision, c.verifier, a.active INTO STRICT fresh
    FROM auth001.installation i JOIN auth001.owner_credentials c USING (owner_actor_id)
    JOIN kehila.actors a ON a.actor_id = i.owner_actor_id WHERE i.singleton;
  IF expected_runtime IS NULL OR expected_auth IS NULL OR expected_runtime = 0
     OR fresh.runtime_generation <> expected_runtime OR fresh.auth_generation <> expected_auth
     OR (expected_revision IS NOT NULL AND fresh.verifier_revision <> expected_revision)
     OR NOT fresh.active OR fresh.verifier IS NULL THEN
    RAISE EXCEPTION 'stale or unavailable authority' USING ERRCODE = '55000';
  END IF;
  RETURN (fresh.installation_id, fresh.owner_actor_id, fresh.runtime_generation,
          fresh.auth_generation, fresh.verifier_revision)::auth001.authority;
END;
$$;

CREATE FUNCTION auth001.start_runtime(requested_installation kehila.ident,
                                     requested_attempt auth001.public_id)
RETURNS auth001.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  fresh auth001.authority;
BEGIN
  IF requested_attempt IS NULL THEN
    RAISE EXCEPTION 'startup attempt required' USING ERRCODE = '23502';
  END IF;
  -- The trusted runtime acquires a SESSION advisory lock once on its dedicated
  -- connection. This verifies the exact calling backend/object, not OS process
  -- identity or session-vs-transaction duration; the owned acquisition path and
  -- its survival across commit/rollback are separate native/Rust obligations.
  IF NOT EXISTS (SELECT FROM pg_catalog.pg_locks l
    WHERE l.pid = pg_catalog.pg_backend_pid() AND l.locktype = 'advisory'
      AND l.database = (SELECT d.oid FROM pg_catalog.pg_database d
                        WHERE d.datname = pg_catalog.current_database())
      AND l.classid = 1262831689::oid AND l.objid = 1::oid AND l.objsubid = 2
      AND l.mode = 'ExclusiveLock' AND l.granted) THEN
    RAISE EXCEPTION 'dedicated advisory acquisition required' USING ERRCODE = '55000';
  END IF;
  PERFORM auth001.lock_owner(requested_installation, true);
  IF EXISTS (SELECT FROM auth001.management_events e
             WHERE e.event_kind = 'runtime_start' AND e.public_id = requested_attempt) THEN
    RAISE EXCEPTION 'startup attempt already completed' USING ERRCODE = '23505';
  END IF;
  UPDATE auth001.installation i SET runtime_generation = i.runtime_generation + 1 WHERE i.singleton;
  SELECT i.installation_id, i.owner_actor_id, i.runtime_generation,
    c.auth_generation, c.verifier_revision INTO STRICT fresh
    FROM auth001.installation i JOIN auth001.owner_credentials c USING (owner_actor_id)
    WHERE i.singleton;
  INSERT INTO auth001.management_events(runtime_generation, public_id, event_kind,
    installation_id, owner_actor_id, auth_generation, verifier_revision, initiator_kind)
    VALUES (fresh.runtime_generation, requested_attempt, 'runtime_start', fresh.installation_id,
            fresh.owner_actor_id, fresh.auth_generation, fresh.verifier_revision, 'runtime');
  RETURN fresh;
END;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA auth001 FROM PUBLIC;
GRANT USAGE ON TYPE auth001.authority TO auth001_runtime;
GRANT EXECUTE ON FUNCTION auth001.fence(kehila.ident, kehila.u64, kehila.u64, kehila.u64),
  auth001.start_runtime(kehila.ident, auth001.public_id) TO auth001_runtime;
COMMIT;
