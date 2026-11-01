-- Disposable A1 overlay only; never an installation or upgrade procedure.
BEGIN;
SET LOCAL lock_timeout = '5s';
SET LOCAL statement_timeout = '15s';
DO $$
BEGIN
  IF current_setting('server_version_num')::integer / 10000 <> 16
     OR current_setting('server_encoding') <> 'UTF8'
     OR NOT EXISTS (SELECT FROM kehila.schema_migrations WHERE version = 1) THEN
    RAISE EXCEPTION 'requires the fresh PostgreSQL 16 UTF8 native foundation';
  END IF;
END;
$$;
CREATE ROLE auth001_admin NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE
  NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE ROLE auth001_runtime NOLOGIN NOSUPERUSER NOCREATEDB NOCREATEROLE
  NOINHERIT NOREPLICATION NOBYPASSRLS;
CREATE SCHEMA auth001 AUTHORIZATION kehila_owner;
SET LOCAL ROLE kehila_owner;
REVOKE ALL ON SCHEMA auth001 FROM PUBLIC;
ALTER DEFAULT PRIVILEGES IN SCHEMA auth001 REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC;

CREATE DOMAIN auth001.public_id AS text COLLATE "C"
  CHECK (octet_length(VALUE) BETWEEN 1 AND 128 AND VALUE ~ '^[!-~]+$');
CREATE DOMAIN auth001.verifier AS text COLLATE "C"
  CHECK (octet_length(VALUE) BETWEEN 1 AND 512 AND VALUE ~ '^[!-~]+$');

CREATE TABLE auth001.installation (
  singleton boolean PRIMARY KEY CHECK (singleton),
  installation_id kehila.ident UNIQUE,
  owner_actor_id kehila.ident UNIQUE REFERENCES kehila.actors(actor_id),
  enrollment_attempt_id auth001.public_id,
  runtime_generation kehila.u64 NOT NULL DEFAULT 0,
  CHECK ((installation_id IS NULL AND owner_actor_id IS NULL AND enrollment_attempt_id IS NULL)
      OR (installation_id IS NOT NULL AND owner_actor_id IS NOT NULL AND enrollment_attempt_id IS NOT NULL))
);
INSERT INTO auth001.installation(singleton) VALUES (true);

CREATE TABLE auth001.owner_credentials (
  singleton boolean PRIMARY KEY CHECK (singleton),
  owner_actor_id kehila.ident NOT NULL UNIQUE REFERENCES auth001.installation(owner_actor_id),
  policy_version integer NOT NULL CHECK (policy_version = 1),
  verifier auth001.verifier,
  auth_generation kehila.u64 NOT NULL CHECK (auth_generation > 0),
  verifier_revision kehila.u64 NOT NULL CHECK (verifier_revision > 0)
);

CREATE TABLE auth001.management_events (
  runtime_generation kehila.u64 NOT NULL,
  public_id auth001.public_id NOT NULL,
  event_kind text COLLATE "C" NOT NULL CHECK (event_kind IN
    ('enrollment','runtime_start','session_issue','logout','session_revoke',
     'password_change','global_revoke','recovery')),
  installation_id kehila.ident NOT NULL REFERENCES auth001.installation(installation_id),
  owner_actor_id kehila.ident NOT NULL REFERENCES auth001.installation(owner_actor_id),
  auth_generation kehila.u64 NOT NULL CHECK (auth_generation > 0),
  verifier_revision kehila.u64 NOT NULL CHECK (verifier_revision > 0),
  credential_kind text COLLATE "C" CHECK (credential_kind IN ('browser','cli')),
  initiator_kind text COLLATE "C" NOT NULL CHECK (initiator_kind IN
    ('local_admin','runtime','owner_session')),
  initiating_session_id auth001.public_id,
  recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
  PRIMARY KEY (runtime_generation, public_id, event_kind),
  CHECK ((event_kind IN ('session_issue','logout','session_revoke')) = (credential_kind IS NOT NULL)),
  CHECK ((initiator_kind = 'owner_session') = (initiating_session_id IS NOT NULL))
);
CREATE UNIQUE INDEX runtime_start_attempt_unique ON auth001.management_events(public_id)
  WHERE event_kind = 'runtime_start';

CREATE FUNCTION auth001.protect_binding() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
BEGIN
  IF TG_OP <> 'UPDATE' THEN
    RAISE EXCEPTION 'installation row cannot be removed' USING ERRCODE = '23514';
  END IF;
  IF OLD.installation_id IS NOT NULL AND
     ROW(NEW.installation_id, NEW.owner_actor_id, NEW.enrollment_attempt_id)
     IS DISTINCT FROM ROW(OLD.installation_id, OLD.owner_actor_id, OLD.enrollment_attempt_id) THEN
    RAISE EXCEPTION 'owner binding is immutable' USING ERRCODE = '23514';
  END IF;
  RETURN NEW;
END;
$$;
CREATE TRIGGER installation_binding_immutable BEFORE UPDATE OR DELETE ON auth001.installation
  FOR EACH ROW EXECUTE FUNCTION auth001.protect_binding();
CREATE TRIGGER installation_not_truncated BEFORE TRUNCATE ON auth001.installation
  FOR EACH STATEMENT EXECUTE FUNCTION auth001.protect_binding();
CREATE TRIGGER management_events_immutable BEFORE UPDATE OR DELETE OR TRUNCATE
  ON auth001.management_events FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_creation_history();

CREATE FUNCTION auth001.enroll(
  requested_installation kehila.ident, requested_owner kehila.ident,
  requested_attempt auth001.public_id, requested_verifier auth001.verifier
) RETURNS TABLE (installation_id kehila.ident, owner_actor_id kehila.ident,
                 runtime_generation kehila.u64, auth_generation kehila.u64, verifier_revision kehila.u64)
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
DECLARE
  installed auth001.installation;
BEGIN
  IF current_setting('transaction_isolation') <> 'read committed' THEN
    RAISE EXCEPTION 'requires READ COMMITTED' USING ERRCODE = '25000';
  END IF;
  IF requested_installation IS NULL OR requested_owner IS NULL
     OR requested_attempt IS NULL OR requested_verifier IS NULL THEN
    RAISE EXCEPTION 'enrollment inputs required' USING ERRCODE = '23502';
  END IF;
  SELECT i.* INTO STRICT installed FROM auth001.installation i WHERE i.singleton FOR UPDATE;
  IF installed.installation_id IS NOT NULL THEN
    RAISE EXCEPTION 'installation already enrolled' USING ERRCODE = '23505';
  END IF;
  INSERT INTO kehila.actors(actor_id, active, can_create_project) VALUES (requested_owner, true, true);
  UPDATE auth001.installation i SET installation_id = requested_installation,
    owner_actor_id = requested_owner, enrollment_attempt_id = requested_attempt WHERE i.singleton;
  INSERT INTO auth001.owner_credentials VALUES (true, requested_owner, 1, requested_verifier, 1, 1);
  INSERT INTO auth001.management_events(runtime_generation, public_id, event_kind, installation_id,
    owner_actor_id, auth_generation, verifier_revision, initiator_kind)
    VALUES (0, requested_attempt, 'enrollment', requested_installation, requested_owner, 1, 1, 'local_admin');
  RETURN QUERY SELECT requested_installation, requested_owner,
    0::kehila.u64, 1::kehila.u64, 1::kehila.u64;
END;
$$;

CREATE FUNCTION auth001.enrollment_status(
  requested_installation kehila.ident, requested_owner kehila.ident, requested_attempt auth001.public_id
) RETURNS TABLE (installation_id kehila.ident, owner_actor_id kehila.ident,
                 runtime_generation kehila.u64, auth_generation kehila.u64, verifier_revision kehila.u64)
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, auth001, kehila, pg_temp AS $$
BEGIN
  RETURN QUERY SELECT i.installation_id, i.owner_actor_id, i.runtime_generation,
      c.auth_generation, c.verifier_revision
    FROM auth001.installation i JOIN auth001.owner_credentials c USING (owner_actor_id)
    WHERE i.installation_id = requested_installation AND i.owner_actor_id = requested_owner
      AND i.enrollment_attempt_id = requested_attempt;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'enrollment claim does not match' USING ERRCODE = '55000';
  END IF;
END;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA auth001 FROM PUBLIC;
GRANT USAGE ON SCHEMA auth001, kehila TO auth001_admin, auth001_runtime;
GRANT USAGE ON TYPE kehila.ident, kehila.u64, auth001.public_id, auth001.verifier
  TO auth001_admin, auth001_runtime;
GRANT EXECUTE ON FUNCTION
  auth001.enroll(kehila.ident, kehila.ident, auth001.public_id, auth001.verifier),
  auth001.enrollment_status(kehila.ident, kehila.ident, auth001.public_id) TO auth001_admin;
COMMIT;
