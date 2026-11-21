-- Preserve the existing project/item completion checks behind one dispatcher.
ALTER FUNCTION kehila.require_product_operation_success(bigint)
  RENAME TO require_project_item_operation_success;
ALTER TABLE kehila.operations DROP CONSTRAINT slice1_operation_family;
ALTER TABLE kehila.operations ADD CONSTRAINT slice1_operation_family CHECK
  (command_family IN ('project_create','item_create','item_edit','selection'));

CREATE FUNCTION kehila.require_product_operation_success(checked_id bigint) RETURNS void
LANGUAGE plpgsql SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
DECLARE core kehila.operations; event kehila.selection_events; payload kehila.operation_payloads;
  canonical bytea;
BEGIN
  IF checked_id IS NULL THEN RETURN; END IF;
  SELECT * INTO STRICT core FROM kehila.operations WHERE operation_row_id=checked_id;
  IF core.command_family<>'selection' THEN
    PERFORM kehila.require_project_item_operation_success(checked_id);
    RETURN;
  END IF;
  SELECT * INTO event FROM kehila.selection_events WHERE operation_row_id=checked_id;
  IF NOT FOUND OR event.user_id IS DISTINCT FROM core.target_user_id
     OR event.actor_id IS DISTINCT FROM core.actor_id OR event.token IS DISTINCT FROM core.token
     OR core.target_kind<>'user' OR core.required_grant<>'selection' OR core.grant_policy_version<>1 THEN
    RAISE EXCEPTION 'incoherent selection completion'
      USING ERRCODE='23503',CONSTRAINT='operation_success_mapping';
  END IF;
  canonical:=kehila.selection_request_bytes(event.user_id,event.token,event.before_version,
    event.selected_project_id,event.selected_item_id);
  IF core.request_codec_version<>1 OR core.request_sha256 IS DISTINCT FROM sha256(canonical) THEN
    RAISE EXCEPTION 'selection core and original command disagree'
      USING ERRCODE='23514',CONSTRAINT='selection_request_agreement';
  END IF;
  IF NOT core.payload_retired THEN
    SELECT * INTO STRICT payload FROM kehila.operation_payloads WHERE operation_row_id=checked_id;
    IF payload.request_bytes IS DISTINCT FROM canonical OR payload.result_codec_version<>1
       OR payload.result IS DISTINCT FROM event.history #>'{value,result}' THEN
      RAISE EXCEPTION 'selection replay and permanent history disagree'
        USING ERRCODE='23514',CONSTRAINT='selection_replay_history_agreement';
    END IF;
  END IF;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'missing selection operation or replay payload'
    USING ERRCODE='23503',CONSTRAINT='operation_success_mapping';
END;
$$;

CREATE FUNCTION kehila.check_selection_operation_success() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path = pg_catalog, kehila_auth, kehila, pg_temp AS $$
BEGIN
  PERFORM kehila.require_product_operation_success(NEW.operation_row_id);
  RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER selection_success_mapping AFTER INSERT ON kehila.selection_events
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION kehila.check_selection_operation_success();
REVOKE ALL ON FUNCTION kehila.require_product_operation_success(bigint),
  kehila.check_selection_operation_success() FROM PUBLIC;
