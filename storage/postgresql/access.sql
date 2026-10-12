-- These functions provide row locks without granting serving UPDATE rights.
-- Callers are trusted backend principals; this is not end-user authentication.
CREATE FUNCTION kehila.lock_actor(kehila.ident) RETURNS SETOF kehila.actors
LANGUAGE sql SECURITY DEFINER
SET search_path = pg_catalog, kehila, pg_temp AS $$
  SELECT a.* FROM kehila.actors a WHERE a.actor_id = $1 FOR NO KEY UPDATE;
$$;
CREATE FUNCTION kehila.lock_operation(bigint) RETURNS SETOF kehila.operations
LANGUAGE sql SECURITY DEFINER
SET search_path = pg_catalog, kehila, pg_temp AS $$
  SELECT o.* FROM kehila.operations o WHERE o.operation_row_id = $1 FOR SHARE;
$$;

CREATE FUNCTION kehila.protect_creation_history() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  RAISE EXCEPTION 'creation history is append-only'
    USING ERRCODE = '23514', CONSTRAINT = 'creation_history_immutable',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER configuration_history_immutable
  BEFORE UPDATE OR DELETE OR TRUNCATE ON kehila.configuration_revisions
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_creation_history();
CREATE TRIGGER grant_history_immutable
  BEFORE UPDATE OR DELETE OR TRUNCATE ON kehila.project_grant_events
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_creation_history();

REVOKE ALL ON ALL FUNCTIONS IN SCHEMA kehila FROM PUBLIC;
GRANT USAGE ON SCHEMA kehila TO kehila_creator;
GRANT USAGE ON TYPE kehila.ident, kehila.u64, kehila.operation_token,
  kehila.project_prefix, kehila.phase TO kehila_creator;
GRANT SELECT ON kehila.actors, kehila.schema_migrations TO kehila_creator;
GRANT SELECT, INSERT ON kehila.operations, kehila.operation_payloads,
  kehila.projects, kehila.configuration_revisions, kehila.project_grants,
  kehila.project_grant_events, kehila.statuses, kehila.workflows,
  kehila.workflow_statuses, kehila.workflow_phase_changes,
  kehila.work_item_types, kehila.type_workflows, kehila.fields TO kehila_creator;
GRANT USAGE ON SEQUENCE kehila.operations_operation_row_id_seq TO kehila_creator;
GRANT EXECUTE ON FUNCTION kehila.lock_actor(kehila.ident),
  kehila.lock_operation(bigint) TO kehila_creator;
