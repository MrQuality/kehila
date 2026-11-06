-- Independent backend after pending-only product routes and one explicit owner seed.
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>2
    OR (SELECT count(*) FROM kehila.items)<>1
    OR (SELECT count(*) FROM kehila.item_revisions)<>1
    OR EXISTS(SELECT FROM kehila.item_field_uses)
    OR (SELECT count(*) FROM kehila.operations)<>3
    OR (SELECT count(*) FROM kehila.operation_payloads)<>2
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>3
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='cancelled' AND request_bytes IS NULL AND operation_row_id IS NULL)<>2
    OR EXISTS(SELECT FROM kehila.projects p LEFT JOIN kehila_auth.pending_items n
      ON n.project_id=p.project_id AND n.state='cancelled'
      GROUP BY p.project_id HAVING count(n.request_id)<>1)
    OR EXISTS(SELECT FROM kehila_auth.pending_items WHERE state='prepared')
    OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
      WHERE p.name='Pending lifecycle' AND p.next_sequence=2 AND NOT p.ever_estimated AND h.state_version=1)
    OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
      WHERE p.name='Other pending scope' AND p.next_sequence=1 AND NOT p.ever_estimated
        AND h.state_version=0 AND h.operation_row_id=p.creation_operation_row_id)
    OR NOT EXISTS(SELECT FROM kehila_auth.pending_items n JOIN kehila.items i
        ON i.creation_request_id=n.request_id AND i.project_id=n.project_id AND i.item_id=n.target_item_id
      JOIN kehila.operations o ON o.operation_row_id=n.operation_row_id
      JOIN kehila.item_revisions r ON r.operation_row_id=o.operation_row_id
      WHERE n.state='completed' AND n.request_bytes IS NULL AND n.target_item_id='read-fixture-1'
        AND n.intent_handle=repeat('0',31)||'1' AND n.token=repeat('0',31)||'1'
        AND n.actor_id=o.actor_id AND o.command_family='item_create'
        AND o.target_kind='work_item' AND o.target_project_id=n.project_id AND o.target_item_id=n.target_item_id
        AND o.token=n.token AND o.request_sha256=n.request_sha256 AND o.request_codec_version=n.request_codec_version
        AND o.required_grant='item_create' AND o.grant_policy_version=1 AND o.payload_retired
        AND NOT EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id)
        AND i.issued_sequence=1 AND i.current_version=1 AND r.item_version=1
        AND r.project_state_version=1 AND r.estimate_thousandths IS NULL)
    OR NOT EXISTS(SELECT FROM kehila_auth.item_pending_policy WHERE singleton AND prepared_limit=32
      AND metadata_limit=64 AND page_limit=16 AND lifetime_seconds=86400)
    OR to_regclass('kehila_native_fixture.clock') IS NOT NULL
    OR to_regclass('kehila_native_fixture.original_clock') IS NOT NULL
    OR to_regprocedure('kehila_native_fixture.reject_pending()') IS NOT NULL THEN
    RAISE EXCEPTION 'pending lifecycle committed witness mismatch';
  END IF;
  IF has_table_privilege('kehila_auth_runtime','kehila_auth.pending_items','SELECT,INSERT,UPDATE,DELETE,TRUNCATE')
    OR NOT has_table_privilege('kehila_auth_runtime','kehila_auth.item_pending_policy','SELECT')
    OR NOT has_function_privilege('kehila_auth_runtime','kehila_auth.save_item_create(kehila_auth.authority,kehila.ident,kehila_auth.intent_key,bytea,kehila_auth.item_preparation,jsonb)','EXECUTE')
    OR has_function_privilege('kehila_auth_runtime','kehila_auth.item_write_fence(kehila_auth.authority,kehila.ident)','EXECUTE')
    OR EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) THEN
    RAISE EXCEPTION 'pending lifecycle privilege or runtime release mismatch';
  END IF;
END $$;
SELECT 'NATIVE_ITEM_PENDING_STATE_PASS';
