-- Independent backend after the isolated owner-seeded, production-read journey.
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>2
    OR (SELECT count(*) FROM kehila.items)<>20
    OR (SELECT count(*) FROM kehila.item_revisions)<>30
    OR (SELECT count(*) FROM kehila.operations)<>32
    OR (SELECT count(*) FROM kehila.operation_payloads)<>32
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='completed')<>20
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>21
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='cancelled' AND target_item_id='factory-target' AND request_bytes IS NULL)<>1
    OR EXISTS(SELECT FROM kehila_auth.pending_items WHERE state='prepared')
    OR (SELECT count(*) FROM kehila.project_operational_heads WHERE state_version=30)<>1
    OR (SELECT count(*) FROM kehila.projects WHERE next_sequence=21 AND ever_estimated)<>1
    OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
      WHERE p.name='Other read scope' AND p.next_sequence=1 AND NOT p.ever_estimated
        AND h.state_version=0 AND h.operation_row_id=p.creation_operation_row_id)
    OR NOT EXISTS(SELECT FROM kehila.item_revisions WHERE item_id='read-fixture-2'
        AND estimate_thousandths=0 AND command_family='item_create')
    OR NOT EXISTS(SELECT FROM kehila.items i JOIN kehila.item_revisions c
        ON c.project_id=i.project_id AND c.item_id=i.item_id AND c.item_version=1
      JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=i.current_version
      JOIN kehila.operations o ON o.operation_row_id=r.operation_row_id
      JOIN kehila.project_operational_heads h ON h.project_id=r.project_id AND h.operation_row_id=r.operation_row_id
      WHERE i.item_id='read-fixture-2' AND i.current_version=11 AND i.issued_sequence=2
        AND i.creation_operation_row_id=c.operation_row_id AND c.estimate_thousandths=0
        AND r.estimate_thousandths IS NULL AND r.previous_item_version=10 AND r.project_state_version=30
        AND o.command_family='item_edit' AND o.target_item_id=i.item_id AND o.token=r.token
        AND r.history#>'{value,result,value,data,estimate}'='null'::jsonb)
    OR EXISTS(SELECT FROM kehila.item_revisions WHERE history#>>'{value,result,value,project,name}'<>'Item read fixture') THEN
    RAISE EXCEPTION 'item read fixture committed state mismatch';
  END IF;
  IF EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database()
      AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) THEN
    RAISE EXCEPTION 'item read fixture retains runtime ownership';
  END IF;
END $$;
SELECT 'NATIVE_ITEM_READ_STATE_PASS';
