-- Independent backend after committed saves and rollback controls.
\set ON_ERROR_STOP on
SET ROLE kehila_owner;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila_auth.pending_items)<>26
     OR (SELECT count(*) FROM kehila.items)<>3 OR (SELECT count(*) FROM kehila.item_revisions)<>9
     OR (SELECT count(*) FROM kehila.operations)<>11 OR (SELECT count(*) FROM kehila.operation_payloads)<>7
     OR (SELECT count(*) FROM kehila.item_field_uses)<>2 THEN
    RAISE EXCEPTION 'save independent cardinality mismatch';
  END IF;
  IF NOT EXISTS(SELECT FROM kehila.items i JOIN kehila_auth.pending_items p ON p.request_id=i.creation_request_id
      WHERE i.item_id='saved-item' AND i.current_version=2 AND i.issued_sequence=3 AND i.issued_prefix='PR'
        AND p.state='completed' AND p.operation_row_id=i.creation_operation_row_id AND p.request_bytes IS NOT NULL)
     OR NOT EXISTS(SELECT FROM kehila.item_revisions WHERE item_id='saved-item' AND item_version=1
       AND type_id='milestone' AND estimate_thousandths=0 AND history #>'{value,result,value,data,values}'=
         '[{"id":"other-title","value":{"kind":"text","value":"new"}}]'::jsonb)
     OR NOT EXISTS(SELECT FROM kehila.item_revisions WHERE item_id='saved-item' AND item_version=2
       AND estimate_thousandths IS NULL AND history #>'{value,result,value,data,values}'='[]'::jsonb)
     OR NOT EXISTS(SELECT FROM kehila.item_field_uses WHERE owner_type_id='milestone' AND field_id='other-title'
       AND first_item_id='saved-item' AND first_item_version=1)
     OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
       WHERE p.project_id='p' AND p.next_sequence=4 AND p.ever_estimated AND h.state_version=9)
     OR EXISTS(SELECT FROM pg_trigger WHERE tgname='save_fault')
     OR EXISTS(SELECT FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
       WHERE n.nspname='kehila_auth' AND p.proname='test_save_fault')
     OR to_regclass('kehila_auth.test_wait_packet') IS NOT NULL THEN
    RAISE EXCEPTION 'save independent identity/history/head/fault-cleanup mismatch';
  END IF;
END;
$$;
COMMIT;
RESET ROLE;
SELECT 'PASS: independent atomic save state, retained use and rollback cleanup' AS result;
