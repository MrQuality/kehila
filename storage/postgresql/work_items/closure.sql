-- Product operation closure; include only in the coordinated fresh v3
-- packet. Preserve the original project-intent closure while adding item cores.

ALTER TABLE kehila.operations DROP CONSTRAINT slice1_operation_family;
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_family CHECK
  (command_family IN ('project_create', 'item_create', 'item_edit'));
ALTER TABLE kehila.operations DROP CONSTRAINT slice1_operation_grant;
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_grant CHECK
  (required_grant = command_family AND grant_policy_version = 1);

CREATE FUNCTION kehila.require_product_operation_success(checked_id bigint)
RETURNS void LANGUAGE plpgsql SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE core kehila.operations; valid boolean;
BEGIN
  IF checked_id IS NULL THEN RETURN; END IF;
  SELECT o.* INTO core FROM kehila.operations o WHERE o.operation_row_id = checked_id;
  IF NOT FOUND THEN
    RAISE EXCEPTION 'missing operation core' USING ERRCODE = '23503',
      CONSTRAINT = 'operation_success_mapping', SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  IF core.command_family = 'project_create' THEN
    -- The unchanged intent/project/pending attribution FKs close this family.
    valid := EXISTS (SELECT FROM kehila_auth.creation_intents c
                      WHERE c.operation_row_id = checked_id);
  ELSIF core.command_family = 'item_create' THEN
    valid := EXISTS (
      SELECT FROM kehila.item_revisions r
        JOIN kehila.items i USING (project_id, item_id)
        JOIN kehila_auth.pending_items p ON p.request_id = i.creation_request_id
          AND p.project_id = i.project_id AND p.target_item_id = i.item_id
      WHERE r.operation_row_id = checked_id AND r.command_family = 'item_create'
        AND r.item_version = 1 AND i.creation_operation_row_id = checked_id
        AND p.state = 'completed' AND p.operation_row_id = checked_id
        AND p.request_codec_version = core.request_codec_version
        AND p.request_sha256 = core.request_sha256);
  ELSIF core.command_family = 'item_edit' THEN
    valid := EXISTS (SELECT FROM kehila.item_revisions r
                      WHERE r.operation_row_id = checked_id AND r.command_family = 'item_edit');
  ELSE
    valid := false;
  END IF;
  IF NOT valid THEN
    RAISE EXCEPTION 'incoherent operation completion' USING ERRCODE = '23503',
      CONSTRAINT = 'operation_success_mapping', SCHEMA = 'kehila', TABLE = 'operations';
  END IF;
  IF core.command_family IN ('item_create', 'item_edit') AND NOT core.payload_retired THEN
    -- Replay and permanent history must describe the same accepted result.
    -- Full typed/canonical reconstruction and history scalar agreement remain
    -- additional checks, not consequences of digest equality alone.
    valid := EXISTS (
      SELECT FROM kehila.operation_payloads p
        JOIN kehila.item_revisions r USING (operation_row_id)
      WHERE p.operation_row_id = checked_id
        AND octet_length(p.request_bytes) BETWEEN 1 AND 1048576
        AND sha256(p.request_bytes) = core.request_sha256
        AND p.result -> 'codec_version' = to_jsonb(p.result_codec_version)
        AND jsonb_typeof(p.result -> 'codec_version') = 'number'
        AND p.result ->> 'codec_version' = p.result_codec_version::text
        AND p.result = r.history #> '{value,result}');
    IF NOT valid THEN
      RAISE EXCEPTION 'incoherent item replay payload' USING ERRCODE = '23514',
        CONSTRAINT = 'item_replay_history_agreement', SCHEMA = 'kehila', TABLE = 'operation_payloads';
    END IF;
  END IF;
END;
$$;

CREATE FUNCTION kehila.check_product_operation_success() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  IF TG_TABLE_NAME = 'items' THEN
    PERFORM kehila.require_product_operation_success(NEW.creation_operation_row_id);
  ELSE
    PERFORM kehila.require_product_operation_success(NEW.operation_row_id);
  END IF;
  RETURN NULL;
END;
$$;

-- Replace, do not merely remove, the unconditional project-only reverse FK.
-- Its SQLSTATE/constraint identity is preserved for the existing orphan oracle.
ALTER TABLE kehila.operations DROP CONSTRAINT operation_success_mapping;
CREATE CONSTRAINT TRIGGER operation_success_mapping
  AFTER INSERT OR UPDATE ON kehila.operations DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION kehila.check_product_operation_success();
CREATE CONSTRAINT TRIGGER item_success_mapping
  AFTER INSERT OR UPDATE ON kehila.items DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION kehila.check_product_operation_success();
CREATE CONSTRAINT TRIGGER item_revision_success_mapping
  AFTER INSERT ON kehila.item_revisions DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION kehila.check_product_operation_success();
CREATE CONSTRAINT TRIGGER pending_item_success_mapping
  AFTER INSERT OR UPDATE ON kehila_auth.pending_items DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION kehila.check_product_operation_success();

REVOKE ALL ON FUNCTION kehila.require_product_operation_success(bigint),
  kehila.check_product_operation_success() FROM PUBLIC;

-- The bound trigger runs as the schema owner so its internal helper stays
-- private. Trigger invocation does not grant the caller ordinary helper EXECUTE.
-- No dynamic SQL, caller-chosen relation or new serving helper grant is used.

-- No explicit row locks here: retirement may already hold the core. Immutable
-- attribution/history and fixed-entry project protection supply the premises.
