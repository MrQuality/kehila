-- Separate-backend observer of committed registration/cancellation effects.
\set ON_ERROR_STOP on
SET statement_timeout = '15s';
SET ROLE kehila_owner;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
DO $$
DECLARE expected bytea := convert_to('{"codec_version":1,"kind":"item_create_request","value":{"estimate":null,"expected_configuration_revision":"1","item_id":"pending-2","operation_id":"00000000000000000000000000000002","project_id":"p","type_id":"task","values":[],"workflow_id":null}}','UTF8');
BEGIN
  IF (SELECT count(*) FROM kehila_auth.pending_items)<>3
     OR (SELECT count(*) FROM kehila.items)<>1
     OR (SELECT count(*) FROM kehila.operations)<>5
     OR (SELECT count(*) FROM kehila.item_revisions)<>4
     OR (SELECT count(*) FROM kehila.operation_payloads)<>1 THEN
    RAISE EXCEPTION 'pending independent cardinalities disagree';
  END IF;
  IF NOT EXISTS (SELECT FROM kehila_auth.pending_items
      WHERE installation_id='installation' AND actor_id='owner' AND project_id='p'
        AND intent_handle='00000000000000000000000000000002'
        AND token='00000000000000000000000000000002' AND target_item_id='pending-2'
        AND request_codec_version=1 AND request_bytes=expected AND request_sha256=sha256(expected)
        AND state='prepared' AND operation_row_id IS NULL
        AND expires_ms-registered_ms=86400000)
     OR NOT EXISTS (SELECT FROM kehila_auth.pending_items
       WHERE target_item_id='pending-1' AND state='cancelled'
         AND request_bytes IS NULL AND operation_row_id IS NULL)
     OR NOT EXISTS (SELECT FROM kehila_auth.pending_items
       WHERE target_item_id='i' AND state='completed'
         AND request_bytes IS NULL AND operation_row_id IS NOT NULL)
     OR NOT EXISTS (SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
       WHERE p.project_id='p' AND p.next_sequence=2 AND p.ever_estimated AND h.state_version=4) THEN
    RAISE EXCEPTION 'pending independent retained state disagrees';
  END IF;
END;
$$;
COMMIT;
RESET ROLE;
SELECT 'PASS: independent committed item registration and cancellation observer' AS result;
