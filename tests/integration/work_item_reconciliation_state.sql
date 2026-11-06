-- Independent committed-state observer after reconciliation controls.
\set ON_ERROR_STOP on
SET ROLE kehila_owner;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila_auth.pending_items)<>27
     OR (SELECT count(*) FROM kehila.projects)<>2 OR (SELECT count(*) FROM kehila.items)<>2
     OR (SELECT count(*) FROM kehila.item_revisions)<>7 OR (SELECT count(*) FROM kehila.operations)<>9
     OR (SELECT count(*) FROM kehila.operation_payloads)<>5 THEN
    RAISE EXCEPTION 'reconciliation independent cardinality mismatch';
  END IF;
  IF NOT EXISTS(SELECT FROM kehila.items WHERE item_id='live-item' AND current_version=3
      AND issued_sequence=2 AND issued_prefix='PR')
     OR NOT EXISTS(SELECT FROM kehila_auth.pending_items p JOIN kehila.items i
       ON i.creation_request_id=p.request_id AND i.creation_operation_row_id=p.operation_row_id
       WHERE p.target_item_id='live-item' AND p.state='completed' AND p.request_bytes IS NOT NULL)
     OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
       WHERE p.project_id='p' AND p.next_sequence=3 AND p.ever_estimated AND NOT p.archived AND h.state_version=7)
     OR NOT EXISTS(SELECT FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id)
       WHERE o.target_item_id='live-item' AND o.token=repeat('7',32) AND NOT o.payload_retired
         AND o.replay_origin_ms=0 AND o.replay_deadline_ms=7776000000)
     OR NOT EXISTS(SELECT FROM kehila.project_grants WHERE project_id='p' AND active AND version=1)
     OR (SELECT count(*) FROM kehila.item_field_uses)<>1
     OR (SELECT count(*) FROM pg_trigger WHERE tgenabled='O' AND (
       (tgrelid='kehila.operations'::regclass AND tgname IN('operation_core_immutable','operation_payload_shape'))
       OR (tgrelid='kehila.operation_payloads'::regclass AND tgname IN('operation_payload_retirement_required','replay_payload_shape'))
       OR (tgrelid='kehila_auth.pending_items'::regclass AND tgname='pending_item_transition')))<>5
     OR NOT EXISTS(SELECT FROM pg_constraint WHERE conrelid='kehila_auth.pending_items'::regclass
       AND conname='pending_item_state_shape' AND convalidated) THEN
    RAISE EXCEPTION 'reconciliation independent retained state mismatch';
  END IF;
END;
$$;
COMMIT;
RESET ROLE;
SELECT 'PASS: reconciliation preserves accepted work and historical attribution' AS result;
