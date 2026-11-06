-- Independent backend after read fixtures and restoration of corruption probes.
\set ON_ERROR_STOP on
SET ROLE kehila_owner;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila_auth.pending_items)<>26
     OR (SELECT count(*) FROM kehila.projects)<>2
     OR (SELECT count(*) FROM kehila.items)<>1
     OR (SELECT count(*) FROM kehila.item_revisions)<>4
     OR (SELECT count(*) FROM kehila.operations)<>6
     OR (SELECT count(*) FROM kehila.operation_payloads)<>2 THEN
    RAISE EXCEPTION 'discovery independent cardinality mismatch';
  END IF;
  IF NOT EXISTS (SELECT FROM kehila_auth.pending_items WHERE target_item_id='read-expired'
      AND project_id='p' AND state='prepared' AND request_bytes IS NOT NULL AND expires_ms=60000)
     OR NOT EXISTS (SELECT FROM kehila_auth.pending_items WHERE target_item_id='read-big'
       AND state='prepared' AND octet_length(request_bytes)=1048576 AND request_sha256=sha256(request_bytes))
     OR NOT EXISTS (SELECT FROM kehila_auth.pending_items WHERE target_item_id='read-other'
       AND project_id='p2' AND state='prepared')
     OR NOT EXISTS (SELECT FROM kehila_auth.pending_items WHERE target_item_id='pending-2'
       AND state='prepared' AND request_bytes IS NOT NULL AND request_sha256=sha256(request_bytes))
     OR NOT EXISTS (SELECT FROM kehila_auth.pending_items WHERE target_item_id='i'
       AND state='completed' AND request_bytes IS NULL AND operation_row_id IS NOT NULL)
     OR NOT EXISTS (SELECT FROM kehila.project_grants WHERE project_id='p' AND active AND version=1)
     OR (SELECT count(*) FROM pg_constraint WHERE conrelid='kehila_auth.pending_items'::regclass
       AND conname IN ('pending_item_state_shape','pending_item_request_bound') AND convalidated)<>2
     OR NOT EXISTS (SELECT FROM pg_trigger WHERE tgrelid='kehila_auth.pending_items'::regclass
       AND tgname='pending_item_transition' AND tgenabled='O') THEN
    RAISE EXCEPTION 'discovery independent state or protection mismatch';
  END IF;
END;
$$;
COMMIT;
RESET ROLE;
SELECT 'PASS: discovery retained state and restored storage protections' AS result;
