\set ON_ERROR_STOP on
SET ROLE kehila_owner;
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila.items)<>1 OR (SELECT count(*) FROM kehila.item_revisions)<>4
    OR (SELECT count(*) FROM kehila.operations)<>5 OR (SELECT count(*) FROM kehila.operation_payloads)<>1
    OR (SELECT count(*) FROM kehila.item_field_uses)<>1
    OR NOT EXISTS (
      SELECT FROM kehila.items i JOIN kehila.project_operational_heads h USING(project_id)
        JOIN kehila.projects p USING(project_id)
        JOIN kehila.item_field_uses u ON u.project_id=i.project_id AND u.owner_type_id='task' AND u.field_id='title'
        JOIN kehila.item_revisions first_use ON first_use.project_id=u.project_id AND first_use.item_id=u.first_item_id
          AND first_use.item_version=u.first_item_version AND first_use.operation_row_id=u.first_operation_row_id
        JOIN kehila.item_revisions current_revision ON current_revision.project_id=i.project_id AND current_revision.item_id=i.item_id
          AND current_revision.item_version=i.current_version AND current_revision.operation_row_id=h.operation_row_id
        JOIN kehila.operations core ON core.operation_row_id=first_use.operation_row_id
      WHERE i.item_id='i' AND i.current_version=4 AND h.state_version=4 AND p.next_sequence=2 AND p.ever_estimated
        AND first_use.item_version=3 AND first_use.history #>'{value,result,value,data,values}'=
          '[{"id":"title","value":{"kind":"text","value":""}}]'::jsonb
        AND current_revision.history #>'{value,result,value,data,values}'='[]'::jsonb
        AND current_revision.estimate_thousandths IS NULL AND current_revision.resulting_ever_estimated
        AND core.payload_retired AND NOT EXISTS
          (SELECT FROM kehila.operation_payloads replay WHERE replay.operation_row_id=first_use.operation_row_id)
    ) THEN RAISE EXCEPTION 'committed field-use history mismatch'; END IF;
END;
$$;
RESET ROLE;
SELECT 'PASS: independent retained field-use and cleared-value observer' AS result;
