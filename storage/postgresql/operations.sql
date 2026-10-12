-- Native PostgreSQL 16 baseline. Installed only through install.sql.

CREATE DOMAIN kehila.ident AS text COLLATE "C"
  CHECK (octet_length(VALUE) BETWEEN 1 AND 128);

CREATE DOMAIN kehila.u64 AS numeric
  CHECK (VALUE = trunc(VALUE)
         AND VALUE BETWEEN 0 AND 18446744073709551615);

CREATE DOMAIN kehila.operation_token AS text COLLATE "C"
  CHECK (octet_length(VALUE) BETWEEN 1 AND 128
         AND VALUE ~ '^[!-~]+$');

CREATE TABLE kehila.actors (
  actor_id kehila.ident PRIMARY KEY,
  active boolean NOT NULL,
  can_create_project boolean NOT NULL DEFAULT false
);

-- Represent known typed targets now; only ProjectCreate is served in this slice.
CREATE TABLE kehila.operations (
  operation_row_id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
  actor_id kehila.ident NOT NULL REFERENCES kehila.actors(actor_id),
  command_family text COLLATE "C" NOT NULL,
  target_kind text COLLATE "C" NOT NULL,
  target_project_id kehila.ident,
  target_item_id kehila.ident,
  target_user_id kehila.ident,
  target_relationship_id kehila.ident,
  token kehila.operation_token NOT NULL,
  request_codec_version bigint NOT NULL
    CHECK (request_codec_version BETWEEN 1 AND 4294967295),
  request_sha256 bytea NOT NULL CHECK (octet_length(request_sha256) = 32),
  required_grant text COLLATE "C" NOT NULL,
  grant_policy_version integer NOT NULL CHECK (grant_policy_version > 0),
  replay_origin_ms kehila.u64 NOT NULL,
  replay_deadline_ms kehila.u64 NOT NULL,
  payload_retired boolean NOT NULL DEFAULT false,
  CONSTRAINT operations_replay_window CHECK (
    replay_deadline_ms = replay_origin_ms + 7776000000
  ),
  CONSTRAINT operations_target_shape CHECK (
    (target_kind = 'project' AND target_project_id IS NOT NULL
      AND target_item_id IS NULL AND target_user_id IS NULL
      AND target_relationship_id IS NULL)
    OR (target_kind = 'work_item' AND target_project_id IS NOT NULL
      AND target_item_id IS NOT NULL AND target_user_id IS NULL
      AND target_relationship_id IS NULL)
    OR (target_kind = 'user' AND target_project_id IS NULL
      AND target_item_id IS NULL AND target_user_id IS NOT NULL
      AND target_relationship_id IS NULL)
    OR (target_kind = 'relationship' AND target_project_id IS NOT NULL
      AND target_item_id IS NULL AND target_user_id IS NULL
      AND target_relationship_id IS NOT NULL)
  ),
  CONSTRAINT operations_family_target CHECK (
    (target_kind = 'project' AND command_family IN
      ('project_create', 'project_metadata', 'project_archive',
       'configuration_change', 'choice_option_admin'))
    OR (target_kind = 'work_item' AND command_family IN
      ('item_create', 'item_edit', 'item_archive', 'item_transition',
       'item_conversion', 'knowledge_create', 'knowledge_edit', 'follow_up_create'))
    OR (target_kind = 'user' AND command_family = 'selection')
    OR (target_kind = 'relationship' AND command_family = 'relationship_create')
  ),
  CONSTRAINT operations_scoped_key UNIQUE NULLS NOT DISTINCT
    (actor_id, command_family, target_kind, target_project_id, target_item_id,
     target_user_id, target_relationship_id, token),
  -- Supports checked attribution from Project/revision/access rows.
  CONSTRAINT operations_attribution UNIQUE
    (operation_row_id, command_family, target_kind, target_project_id, actor_id)
);

-- Temporary serving-slice guards, not permanent domain vocabulary.
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_family
  CHECK (command_family = 'project_create');
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_grant
  CHECK (required_grant = 'project_create');

CREATE TABLE kehila.operation_payloads (
  operation_row_id bigint PRIMARY KEY
    REFERENCES kehila.operations(operation_row_id) ON DELETE RESTRICT,
  request_bytes bytea NOT NULL,
  result_codec_version bigint NOT NULL
    CHECK (result_codec_version BETWEEN 1 AND 4294967295),
  result jsonb NOT NULL CHECK (jsonb_typeof(result) = 'object')
);

CREATE INDEX operations_full_expiry
  ON kehila.operations(replay_deadline_ms)
  WHERE NOT payload_retired;

CREATE FUNCTION kehila.check_operation_payload()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE
  checked_id bigint;
  retired boolean;
  has_payload boolean;
BEGIN
  IF TG_OP = 'DELETE' THEN
    checked_id := OLD.operation_row_id;
  ELSE
    checked_id := NEW.operation_row_id;
  END IF;
  SELECT o.payload_retired,
         EXISTS (SELECT 1 FROM kehila.operation_payloads p
                 WHERE p.operation_row_id = o.operation_row_id)
    INTO retired, has_payload
    FROM kehila.operations o
    WHERE o.operation_row_id = checked_id;
  IF NOT FOUND OR retired = has_payload THEN
    RAISE EXCEPTION 'incoherent operation payload'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_payload_shape',
            SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  RETURN NULL;
END;
$$;

CREATE FUNCTION kehila.protect_payload_update()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  IF NEW.operation_row_id IS DISTINCT FROM OLD.operation_row_id THEN
    RAISE EXCEPTION 'operation payload identity is immutable'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_payload_identity_immutable',
            SCHEMA = 'kehila', TABLE = 'operation_payloads';
  END IF;
  RAISE EXCEPTION 'operation payloads cannot be updated'
    USING ERRCODE = '23514', CONSTRAINT = 'operation_payload_immutable',
          SCHEMA = 'kehila', TABLE = 'operation_payloads';
END;
$$;

CREATE TRIGGER operation_payload_immutable
  BEFORE UPDATE ON kehila.operation_payloads FOR EACH ROW
  EXECUTE FUNCTION kehila.protect_payload_update();

-- The irreversible core flag must change first in the same transaction.
-- Otherwise DELETE + INSERT could replace content while satisfying final shape.
CREATE FUNCTION kehila.require_payload_retirement()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  IF NOT EXISTS (SELECT FROM kehila.operations
                 WHERE operation_row_id = OLD.operation_row_id AND payload_retired) THEN
    RAISE EXCEPTION 'retire the operation before deleting its payload'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_payload_retirement_required',
            SCHEMA = 'kehila', TABLE = 'operation_payloads';
  END IF;
  RETURN OLD;
END;
$$;

CREATE TRIGGER operation_payload_retirement_required
  BEFORE DELETE ON kehila.operation_payloads FOR EACH ROW
  EXECUTE FUNCTION kehila.require_payload_retirement();

CREATE CONSTRAINT TRIGGER operation_payload_shape
  AFTER INSERT OR UPDATE OR DELETE ON kehila.operations
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_operation_payload();

CREATE CONSTRAINT TRIGGER replay_payload_shape
  AFTER INSERT OR UPDATE OR DELETE ON kehila.operation_payloads
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_operation_payload();

CREATE FUNCTION kehila.protect_operation_core()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  IF TG_OP = 'INSERT' THEN
    IF NEW.payload_retired THEN
      RAISE EXCEPTION 'new operation requires a replay payload'
        USING ERRCODE = '23514', CONSTRAINT = 'operation_core_fresh',
              SCHEMA = 'kehila', TABLE = 'operations';
    END IF;
    RETURN NEW;
  END IF;
  IF TG_OP = 'DELETE' THEN
    RAISE EXCEPTION 'operation identity is permanent'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_core_immutable',
            SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  IF (to_jsonb(NEW) - 'payload_retired') IS DISTINCT FROM
     (to_jsonb(OLD) - 'payload_retired')
     OR OLD.payload_retired OR NOT NEW.payload_retired THEN
    RAISE EXCEPTION 'invalid operation core update'
      USING ERRCODE = '23514', CONSTRAINT = 'operation_core_immutable',
            SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  RETURN NEW;
END;
$$;

CREATE TRIGGER operation_core_immutable
  BEFORE INSERT OR UPDATE OR DELETE ON kehila.operations FOR EACH ROW
  EXECUTE FUNCTION kehila.protect_operation_core();

CREATE FUNCTION kehila.reject_operation_truncate()
RETURNS trigger LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  RAISE EXCEPTION 'operation storage cannot be truncated'
    USING ERRCODE = '23514', CONSTRAINT = 'operation_storage_no_truncate',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER operation_core_no_truncate
  BEFORE TRUNCATE ON kehila.operations FOR EACH STATEMENT
  EXECUTE FUNCTION kehila.reject_operation_truncate();
CREATE TRIGGER operation_payload_no_truncate
  BEFORE TRUNCATE ON kehila.operation_payloads FOR EACH STATEMENT
  EXECUTE FUNCTION kehila.reject_operation_truncate();
