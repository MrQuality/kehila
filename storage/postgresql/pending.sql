-- Included only by the fresh product installer, as kehila_owner.
-- Runtime is a trusted backend capability, never an end-user SQL principal.
CREATE DOMAIN kehila_auth.intent_key AS text COLLATE "C"
  CHECK (octet_length(VALUE) = 32 AND VALUE ~ '^[0-9a-f]{32}$');

ALTER TABLE kehila_auth.installation ADD CONSTRAINT installation_owner_pair
  UNIQUE (installation_id, owner_actor_id);
ALTER TABLE kehila.operations ADD CONSTRAINT operation_token_reference
  UNIQUE (operation_row_id, token);

CREATE TABLE kehila_auth.pending_policy (
  singleton boolean PRIMARY KEY CHECK (singleton),
  prepared_limit integer NOT NULL CHECK (prepared_limit BETWEEN 1 AND 64),
  page_limit integer NOT NULL CHECK (page_limit BETWEEN 1 AND 32),
  lifetime_seconds integer NOT NULL CHECK (lifetime_seconds BETWEEN 60 AND 604800)
);
INSERT INTO kehila_auth.pending_policy VALUES (true, 32, 16, 86400);

CREATE TABLE kehila_auth.pending_projects (
  request_id bigint GENERATED ALWAYS AS IDENTITY (MINVALUE 1 NO CYCLE) PRIMARY KEY,
  installation_id kehila.ident NOT NULL,
  actor_id kehila.ident NOT NULL,
  command_family text COLLATE "C" NOT NULL DEFAULT 'project_create'
    CHECK (command_family = 'project_create'),
  intent_handle kehila_auth.intent_key NOT NULL,
  token kehila_auth.intent_key NOT NULL,
  form_version integer NOT NULL DEFAULT 1 CHECK (form_version = 1),
  name text,
  prefix kehila.project_prefix,
  estimate_unit text COLLATE "C",
  registered_ms kehila.u64 NOT NULL,
  expires_ms kehila.u64 NOT NULL,
  state text COLLATE "C" NOT NULL DEFAULT 'prepared'
    CHECK (state IN ('prepared','completed','cancelled','expired')),
  target_project_id kehila.ident,
  operation_row_id bigint UNIQUE,
  CONSTRAINT pending_positive_id CHECK (request_id > 0),
  CONSTRAINT pending_lifetime CHECK (expires_ms > registered_ms
    AND expires_ms - registered_ms BETWEEN 60000 AND 604800000),
  CONSTRAINT pending_owner FOREIGN KEY (installation_id, actor_id)
    REFERENCES kehila_auth.installation(installation_id, owner_actor_id),
  CONSTRAINT pending_handle UNIQUE (actor_id, command_family, intent_handle),
  CONSTRAINT pending_completion_tuple UNIQUE
    (request_id, actor_id, command_family, intent_handle, target_project_id, operation_row_id),
  CONSTRAINT pending_form CHECK (
    (name IS NULL AND prefix IS NULL AND estimate_unit IS NULL)
    OR (name IS NOT NULL AND octet_length(name) BETWEEN 1 AND 256
      AND prefix IS NOT NULL AND estimate_unit IS NOT NULL
      AND estimate_unit IN ('points','hours'))),
  CONSTRAINT pending_state_shape CHECK (
    (state = 'prepared' AND name IS NOT NULL
      AND target_project_id IS NULL AND operation_row_id IS NULL)
    OR (state IN ('cancelled','expired') AND name IS NULL
      AND target_project_id IS NULL AND operation_row_id IS NULL)
    OR (state = 'completed' AND target_project_id IS NOT NULL AND operation_row_id IS NOT NULL)),
  CONSTRAINT pending_core_token FOREIGN KEY (operation_row_id, token)
    REFERENCES kehila.operations(operation_row_id, token)
);
-- Hard-ceiling maintenance never scans successful history.
CREATE INDEX pending_never_created ON kehila_auth.pending_projects(actor_id, request_id)
  WHERE state <> 'completed';
CREATE INDEX pending_discovery ON kehila_auth.pending_projects(actor_id, request_id DESC);

CREATE TABLE kehila_auth.creation_intents (
  request_id bigint NOT NULL UNIQUE,
  actor_id kehila.ident NOT NULL,
  command_family text COLLATE "C" NOT NULL CHECK (command_family = 'project_create'),
  intent_handle kehila_auth.intent_key NOT NULL,
  target_kind text COLLATE "C" NOT NULL CHECK (target_kind = 'project'),
  target_project_id kehila.ident NOT NULL UNIQUE,
  operation_row_id bigint NOT NULL UNIQUE,
  CONSTRAINT creation_intent_identity PRIMARY KEY (actor_id, command_family, intent_handle),
  CONSTRAINT creation_intent_tuple UNIQUE
    (request_id, actor_id, command_family, intent_handle, target_project_id, operation_row_id),
  CONSTRAINT creation_intent_attribution FOREIGN KEY
    (operation_row_id, command_family, target_kind, target_project_id, actor_id)
    REFERENCES kehila.operations
      (operation_row_id, command_family, target_kind, target_project_id, actor_id),
  CONSTRAINT creation_intent_actual_creator FOREIGN KEY (operation_row_id)
    REFERENCES kehila.projects(creation_operation_row_id),
  CONSTRAINT creation_intent_pending FOREIGN KEY
    (request_id, actor_id, command_family, intent_handle, target_project_id, operation_row_id)
    REFERENCES kehila_auth.pending_projects
      (request_id, actor_id, command_family, intent_handle, target_project_id, operation_row_id)
    DEFERRABLE INITIALLY DEFERRED
);
ALTER TABLE kehila_auth.pending_projects ADD CONSTRAINT pending_success_mapping
  FOREIGN KEY
    (request_id, actor_id, command_family, intent_handle, target_project_id, operation_row_id)
  REFERENCES kehila_auth.creation_intents
    (request_id, actor_id, command_family, intent_handle, target_project_id, operation_row_id)
  DEFERRABLE INITIALLY DEFERRED;
-- The product must not commit a project while omitting pending completion.
ALTER TABLE kehila.projects ADD CONSTRAINT project_success_mapping
  FOREIGN KEY (creation_operation_row_id)
  REFERENCES kehila_auth.creation_intents(operation_row_id)
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE kehila.operations ADD CONSTRAINT operation_success_mapping
  FOREIGN KEY (operation_row_id)
  REFERENCES kehila_auth.creation_intents(operation_row_id)
  DEFERRABLE INITIALLY DEFERRED;

CREATE FUNCTION kehila_auth.trusted_millis() RETURNS kehila.u64
LANGUAGE plpgsql VOLATILE SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE value numeric;
BEGIN
  value := floor(extract(epoch FROM clock_timestamp()) * 1000);
  IF value < 0 OR value > 18446744073709551615 OR value::text IN ('NaN','Infinity','-Infinity') THEN
    RAISE EXCEPTION 'trusted clock out of range' USING ERRCODE = '22003';
  END IF;
  RETURN value::kehila.u64;
END;
$$;

CREATE FUNCTION kehila_auth.project_fence(expected kehila_auth.authority)
RETURNS kehila_auth.authority LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  fresh := kehila_auth.fence(expected.installation_id, expected.runtime_generation,
    expected.auth_generation, expected.verifier_revision);
  IF fresh.owner_actor_id IS DISTINCT FROM expected.owner_actor_id
     OR NOT EXISTS (SELECT FROM kehila.actors a
       WHERE a.actor_id = fresh.owner_actor_id AND a.active AND a.can_create_project) THEN
    RAISE EXCEPTION 'project creation authority denied' USING ERRCODE = '42501';
  END IF;
  RETURN fresh;
END;
$$;

CREATE FUNCTION kehila_auth.protect_pending() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  IF TG_OP = 'TRUNCATE' THEN
    RAISE EXCEPTION 'pending identity sequence must not be reset' USING ERRCODE = '23514';
  ELSIF TG_OP = 'INSERT' THEN
    IF NEW.state <> 'prepared' THEN
      RAISE EXCEPTION 'new pending request must be prepared' USING ERRCODE = '23514';
    END IF;
    -- Also enforce the ceiling for owner-authored fixtures. No serving INSERT grant.
    PERFORM kehila_auth.lock_owner(NEW.installation_id, false);
    IF (SELECT count(*) FROM (SELECT 1 FROM kehila_auth.pending_projects p
      WHERE p.actor_id = NEW.actor_id AND p.state <> 'completed' LIMIT 64) bounded) >= 64 THEN
      RAISE EXCEPTION 'pending metadata ceiling' USING ERRCODE = '54000';
    END IF;
    RETURN NEW;
  ELSIF TG_OP = 'DELETE' THEN
    IF OLD.state NOT IN ('cancelled','expired') THEN
      RAISE EXCEPTION 'pending identity cannot be removed' USING ERRCODE = '23514';
    END IF;
    RETURN OLD;
  END IF;
  IF (to_jsonb(NEW) - ARRAY['state','name','prefix','estimate_unit','target_project_id','operation_row_id'])
      IS DISTINCT FROM
     (to_jsonb(OLD) - ARRAY['state','name','prefix','estimate_unit','target_project_id','operation_row_id']) THEN
    RAISE EXCEPTION 'pending request identity is immutable' USING ERRCODE = '23514';
  END IF;
  IF OLD.state = 'prepared' AND NEW.state IN ('completed','cancelled','expired') THEN
    IF NEW.state = 'completed' AND ROW(NEW.name,NEW.prefix,NEW.estimate_unit)
       IS DISTINCT FROM ROW(OLD.name,OLD.prefix,OLD.estimate_unit) THEN
      RAISE EXCEPTION 'completion cannot change form' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
  END IF;
  IF OLD.state = 'completed' AND NEW.state = 'completed' AND OLD.name IS NOT NULL
    AND NEW.name IS NULL AND NEW.prefix IS NULL AND NEW.estimate_unit IS NULL
    AND ROW(NEW.target_project_id,NEW.operation_row_id)
      IS NOT DISTINCT FROM ROW(OLD.target_project_id,OLD.operation_row_id) THEN
    RETURN NEW;
  END IF;
  RAISE EXCEPTION 'invalid pending transition' USING ERRCODE = '23514';
END;
$$;
CREATE TRIGGER pending_transition BEFORE INSERT OR UPDATE OR DELETE
  ON kehila_auth.pending_projects FOR EACH ROW EXECUTE FUNCTION kehila_auth.protect_pending();
CREATE TRIGGER pending_no_truncate BEFORE TRUNCATE ON kehila_auth.pending_projects
  FOR EACH STATEMENT EXECUTE FUNCTION kehila_auth.protect_pending();
CREATE TRIGGER creation_intent_immutable BEFORE UPDATE OR DELETE OR TRUNCATE
  ON kehila_auth.creation_intents FOR EACH STATEMENT
  EXECUTE FUNCTION kehila.protect_creation_history();

CREATE FUNCTION kehila_auth.check_completed_content() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE checked_id bigint;
BEGIN
  checked_id := NEW.operation_row_id;
  IF checked_id IS NOT NULL AND EXISTS (
    SELECT FROM kehila_auth.pending_projects p JOIN kehila.operations o USING (operation_row_id)
    WHERE p.operation_row_id = checked_id AND p.state = 'completed'
      AND ((p.name IS NULL) IS DISTINCT FROM o.payload_retired)) THEN
    RAISE EXCEPTION 'completed form retirement disagrees with core'
      USING ERRCODE = '23514', CONSTRAINT = 'completed_content_retirement';
  END IF;
  RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER pending_content_retirement
  AFTER INSERT OR UPDATE ON kehila_auth.pending_projects DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION kehila_auth.check_completed_content();
CREATE CONSTRAINT TRIGGER core_pending_retirement
  AFTER UPDATE ON kehila.operations DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION kehila_auth.check_completed_content();

-- Private bounded maintenance. Call only after project_fence; the actor lock
-- serializes every insertion and transition of this owner's prepared set.
CREATE FUNCTION kehila_auth.expire_pending(owner_id kehila.ident, at_ms kehila.u64)
RETURNS void LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  UPDATE kehila_auth.pending_projects p SET state = 'expired',
    name = NULL, prefix = NULL, estimate_unit = NULL
    WHERE p.actor_id = owner_id AND p.state <> 'completed'
      AND p.state = 'prepared' AND p.expires_ms <= at_ms;
END;
$$;

CREATE FUNCTION kehila_auth.register_project(
  expected kehila_auth.authority, handle kehila_auth.intent_key,
  request_token kehila_auth.intent_key, form_name text,
  form_prefix kehila.project_prefix, form_unit text
) RETURNS SETOF kehila_auth.pending_projects
LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; at_ms kehila.u64; policy kehila_auth.pending_policy;
BEGIN
  fresh := kehila_auth.project_fence(expected);
  at_ms := kehila_auth.trusted_millis();
  SELECT * INTO STRICT policy FROM kehila_auth.pending_policy WHERE singleton;
  PERFORM kehila_auth.expire_pending(fresh.owner_actor_id, at_ms);
  DELETE FROM kehila_auth.pending_projects p WHERE p.actor_id = fresh.owner_actor_id
    AND p.state <> 'completed' AND p.state IN ('cancelled','expired');
  IF (SELECT count(*) FROM kehila_auth.pending_projects p
      WHERE p.actor_id = fresh.owner_actor_id AND p.state <> 'completed') >= policy.prepared_limit THEN
    RAISE EXCEPTION 'prepared request quota' USING ERRCODE = '54000';
  END IF;
  RETURN QUERY INSERT INTO kehila_auth.pending_projects
    (installation_id,actor_id,intent_handle,token,name,prefix,estimate_unit,registered_ms,expires_ms)
    VALUES (fresh.installation_id,fresh.owner_actor_id,handle,request_token,
      form_name,form_prefix,form_unit,at_ms,(at_ms + policy.lifetime_seconds * 1000)::kehila.u64)
    RETURNING *;
END;
$$;

CREATE FUNCTION kehila_auth.list_projects(expected kehila_auth.authority, before_id bigint)
RETURNS SETOF kehila_auth.pending_projects LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; selected_limit integer;
BEGIN
  IF before_id IS NOT NULL AND before_id <= 0 THEN
    RAISE EXCEPTION 'invalid request number' USING ERRCODE = '22023';
  END IF;
  fresh := kehila_auth.project_fence(expected);
  PERFORM kehila_auth.expire_pending(fresh.owner_actor_id, kehila_auth.trusted_millis());
  SELECT page_limit INTO STRICT selected_limit FROM kehila_auth.pending_policy WHERE singleton;
  IF before_id IS NULL THEN
    RETURN QUERY SELECT p.* FROM kehila_auth.pending_projects p
      WHERE p.actor_id = fresh.owner_actor_id
      ORDER BY p.request_id DESC LIMIT selected_limit + 1;
  ELSE
    RETURN QUERY SELECT p.* FROM kehila_auth.pending_projects p
      WHERE p.actor_id = fresh.owner_actor_id AND p.request_id < before_id
      ORDER BY p.request_id DESC LIMIT selected_limit + 1;
  END IF;
END;
$$;

CREATE FUNCTION kehila_auth.open_project(expected kehila_auth.authority, requested_id bigint)
RETURNS SETOF kehila_auth.pending_projects LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  fresh := kehila_auth.project_fence(expected);
  PERFORM kehila_auth.expire_pending(fresh.owner_actor_id, kehila_auth.trusted_millis());
  RETURN QUERY SELECT p.* FROM kehila_auth.pending_projects p
    WHERE p.actor_id = fresh.owner_actor_id AND p.request_id = requested_id FOR UPDATE;
END;
$$;

CREATE FUNCTION kehila_auth.cancel_project(expected kehila_auth.authority, requested_id bigint)
RETURNS SETOF kehila_auth.pending_projects LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority;
BEGIN
  fresh := kehila_auth.project_fence(expected);
  PERFORM kehila_auth.expire_pending(fresh.owner_actor_id, kehila_auth.trusted_millis());
  UPDATE kehila_auth.pending_projects p SET state = 'cancelled',
      name = NULL, prefix = NULL, estimate_unit = NULL
    WHERE p.actor_id = fresh.owner_actor_id AND p.request_id = requested_id AND p.state = 'prepared';
  -- Includes permanent completed identity. Never compensate or delete a project.
  RETURN QUERY SELECT p.* FROM kehila_auth.pending_projects p
    WHERE p.actor_id = fresh.owner_actor_id AND p.request_id = requested_id;
END;
$$;

CREATE FUNCTION kehila_auth.complete_project(
  expected kehila_auth.authority, requested_id bigint, request_token kehila_auth.intent_key,
  form_name text, form_prefix kehila.project_prefix, form_unit text,
  created_project kehila.ident, created_operation bigint
) RETURNS void LANGUAGE plpgsql VOLATILE SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE fresh kehila_auth.authority; pending kehila_auth.pending_projects; at_ms kehila.u64;
BEGIN
  fresh := kehila_auth.project_fence(expected);
  SELECT p.* INTO STRICT pending FROM kehila_auth.pending_projects p
    WHERE p.actor_id = fresh.owner_actor_id AND p.request_id = requested_id FOR UPDATE;
  at_ms := kehila_auth.trusted_millis();
  -- This is execution's final eligibility ordering point. An expired request
  -- rolls back the entire creation; the caller must not commit partial writes.
  IF pending.state <> 'prepared' OR pending.expires_ms <= at_ms THEN
    RAISE EXCEPTION 'pending request no longer executable' USING ERRCODE = 'P0002';
  END IF;
  IF ROW(pending.token,pending.name,pending.prefix,pending.estimate_unit)
      IS DISTINCT FROM ROW(request_token,form_name,form_prefix,form_unit) THEN
    RAISE EXCEPTION 'pending request content conflict' USING ERRCODE = '23000';
  END IF;
  UPDATE kehila_auth.pending_projects SET state = 'completed',
    target_project_id = created_project, operation_row_id = created_operation
    WHERE request_id = requested_id;
  INSERT INTO kehila_auth.creation_intents
    (request_id,actor_id,command_family,intent_handle,target_kind,target_project_id,operation_row_id)
    VALUES (requested_id,fresh.owner_actor_id,'project_create',pending.intent_handle,
      'project',created_project,created_operation);
END;
$$;

-- Owner-only maintenance primitive; not granted to the serving or enrollment
-- roles. The installation -> actor -> operation order matches serving replay.
CREATE FUNCTION kehila_auth.retire_project_payload(requested_installation kehila.ident, requested_operation bigint)
RETURNS void LANGUAGE plpgsql VOLATILE
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE installed_owner kehila.ident; core kehila.operations; at_ms kehila.u64;
BEGIN
  PERFORM kehila_auth.lock_owner(requested_installation, false);
  SELECT owner_actor_id INTO STRICT installed_owner FROM kehila_auth.installation
    WHERE singleton AND installation_id = requested_installation;
  SELECT * INTO STRICT core FROM kehila.operations o
    WHERE o.operation_row_id = requested_operation AND o.actor_id = installed_owner FOR UPDATE;
  at_ms := kehila_auth.trusted_millis();
  IF core.replay_deadline_ms > at_ms THEN
    RAISE EXCEPTION 'replay is still eligible' USING ERRCODE = '55000';
  END IF;
  IF core.payload_retired THEN RETURN; END IF;
  UPDATE kehila.operations SET payload_retired = true WHERE operation_row_id = requested_operation;
  DELETE FROM kehila.operation_payloads WHERE operation_row_id = requested_operation;
  UPDATE kehila_auth.pending_projects SET name = NULL, prefix = NULL, estimate_unit = NULL
    WHERE operation_row_id = requested_operation AND state = 'completed';
END;
$$;

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA kehila_auth FROM PUBLIC;
GRANT USAGE ON TYPE kehila_auth.intent_key TO kehila_auth_runtime;
GRANT SELECT ON kehila_auth.pending_projects, kehila_auth.creation_intents,
  kehila_auth.pending_policy TO kehila_auth_runtime;
GRANT EXECUTE ON FUNCTION
  kehila_auth.trusted_millis(),
  kehila_auth.register_project(kehila_auth.authority,kehila_auth.intent_key,kehila_auth.intent_key,text,kehila.project_prefix,text),
  kehila_auth.list_projects(kehila_auth.authority,bigint),
  kehila_auth.open_project(kehila_auth.authority,bigint),
  kehila_auth.cancel_project(kehila_auth.authority,bigint),
  kehila_auth.complete_project(kehila_auth.authority,bigint,kehila_auth.intent_key,text,kehila.project_prefix,text,kehila.ident,bigint)
  TO kehila_auth_runtime;
