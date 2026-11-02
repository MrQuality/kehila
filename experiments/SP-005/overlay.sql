-- Disposable physical-mapping candidate only. No product schema migration.
\set ON_ERROR_STOP on
BEGIN;
SET LOCAL lock_timeout = '2s';
SET LOCAL statement_timeout = '10s';
DO $$
BEGIN
  IF current_setting('server_version_num')::integer / 10000 <> 16
     OR current_setting('server_encoding') <> 'UTF8'
     OR NOT EXISTS (SELECT FROM kehila.schema_migrations WHERE version = 1) THEN
    RAISE EXCEPTION 'requires the native PostgreSQL 16 UTF8 foundation';
  END IF;
END;
$$;
CREATE SCHEMA sp005 AUTHORIZATION kehila_owner;
SET LOCAL ROLE kehila_owner;
REVOKE ALL ON SCHEMA sp005 FROM PUBLIC;
ALTER DEFAULT PRIVILEGES IN SCHEMA sp005 REVOKE EXECUTE ON FUNCTIONS FROM PUBLIC;

CREATE TABLE sp005.creation_intents (
  actor_id kehila.ident NOT NULL,
  command_family text COLLATE "C" NOT NULL DEFAULT 'project_create',
  intent_handle text COLLATE "C" NOT NULL,
  target_kind text COLLATE "C" NOT NULL DEFAULT 'project',
  target_project_id kehila.ident NOT NULL,
  operation_row_id bigint NOT NULL,
  CONSTRAINT sp005_intent_family CHECK (command_family = 'project_create'),
  CONSTRAINT sp005_intent_kind CHECK (target_kind = 'project'),
  CONSTRAINT sp005_intent_handle CHECK (
    octet_length(intent_handle) = 32 AND intent_handle ~ '^[0-9a-f]{32}$'),
  CONSTRAINT sp005_intent_identity PRIMARY KEY (actor_id, command_family, intent_handle),
  CONSTRAINT sp005_intent_target UNIQUE (target_project_id),
  CONSTRAINT sp005_intent_attribution FOREIGN KEY
    (operation_row_id, command_family, target_kind, target_project_id, actor_id)
    REFERENCES kehila.operations
      (operation_row_id, command_family, target_kind, target_project_id, actor_id)
    ON DELETE RESTRICT,
  -- Together with attribution, this binds to this project's actual creator,
  -- not merely some well-formed core targeting an already existing project.
  CONSTRAINT sp005_intent_creation FOREIGN KEY (operation_row_id)
    REFERENCES kehila.projects(creation_operation_row_id) ON DELETE RESTRICT
);

CREATE FUNCTION sp005.protect_intent() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, sp005, kehila, pg_temp AS $$
BEGIN
  RAISE EXCEPTION 'creation intent is immutable'
    USING ERRCODE = '23514', CONSTRAINT = 'sp005_intent_immutable',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER sp005_intent_immutable
  BEFORE UPDATE OR DELETE OR TRUNCATE ON sp005.creation_intents
  FOR EACH STATEMENT EXECUTE FUNCTION sp005.protect_intent();
REVOKE ALL ON ALL FUNCTIONS IN SCHEMA sp005 FROM PUBLIC;
GRANT USAGE ON SCHEMA sp005 TO kehila_creator;
GRANT SELECT, INSERT ON sp005.creation_intents TO kehila_creator;
COMMIT;
