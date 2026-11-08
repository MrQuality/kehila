-- Independent witness after repeated invalidation and original-identity recovery.
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>1
    OR (SELECT count(*) FROM kehila.items)<>2
    OR (SELECT count(*) FROM kehila.item_revisions)<>5
    OR (SELECT count(*) FROM kehila.operations)<>6
    OR (SELECT count(*) FROM kehila.operation_payloads)<>6
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>2
    OR EXISTS(SELECT FROM kehila_auth.pending_items WHERE state<>'completed')
    OR EXISTS(SELECT FROM kehila.operations WHERE payload_retired)
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='project_create')<>1
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='item_create')<>2
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='item_edit')<>3
    OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
      JOIN kehila.item_revisions r ON r.operation_row_id=h.operation_row_id
      JOIN kehila.items i ON i.project_id=r.project_id AND i.item_id=r.item_id
      WHERE p.name='Retry exhaustion' AND p.current_prefix='RTE' AND p.next_sequence=3
        AND p.configuration_revision=1 AND p.ever_estimated
        AND h.state_version=5 AND r.item_version=1 AND i.issued_sequence=2)
    OR (SELECT count(*) FROM kehila.items WHERE current_version=4 AND issued_sequence=1)<>1
    OR (SELECT count(*) FROM kehila.items WHERE current_version=1 AND issued_sequence=2)<>1 THEN
    RAISE EXCEPTION 'retry exhaustion/recovery cardinality or state mismatch';
  END IF;
  IF EXISTS(SELECT FROM generate_series(1,3) n WHERE NOT EXISTS(
      SELECT FROM kehila.item_revisions r JOIN kehila.operations o USING(operation_row_id)
      JOIN kehila.operation_payloads b USING(operation_row_id)
      JOIN kehila.items i ON i.project_id=r.project_id AND i.item_id=r.item_id
      WHERE r.token=lpad(to_hex(n),32,'0') AND r.item_version=n+1
        AND r.estimate_thousandths=n AND r.command_family='item_edit' AND i.issued_sequence=1
        AND o.token=r.token AND o.target_item_id=r.item_id AND o.actor_id=r.actor_id
        AND sha256(b.request_bytes)=o.request_sha256 AND b.result=r.history#>'{value,result}'))
    OR EXISTS(SELECT FROM kehila.operations WHERE token=repeat('e',32))
    OR EXISTS(SELECT FROM kehila_auth.pending_items p
      LEFT JOIN kehila.items i ON i.creation_request_id=p.request_id
      LEFT JOIN kehila.operations o ON o.operation_row_id=p.operation_row_id
      LEFT JOIN kehila.operation_payloads b ON b.operation_row_id=p.operation_row_id
      LEFT JOIN kehila.item_revisions r ON r.operation_row_id=p.operation_row_id
      WHERE i.item_id IS NULL OR o.operation_row_id IS NULL OR b.operation_row_id IS NULL
        OR r.operation_row_id IS NULL OR p.target_item_id<>i.item_id OR p.token<>o.token
        OR p.project_id<>o.target_project_id OR p.actor_id<>o.actor_id OR p.actor_id<>'owner'
        OR p.target_item_id<>o.target_item_id OR o.command_family<>'item_create'
        OR i.creation_operation_row_id<>o.operation_row_id OR r.item_version<>1
        OR r.item_id<>p.target_item_id OR r.token<>p.token OR r.actor_id<>p.actor_id
        OR p.request_codec_version<>1 OR p.request_bytes IS NULL
        OR p.request_sha256<>sha256(p.request_bytes) OR p.request_sha256<>o.request_sha256
        OR p.request_bytes<>b.request_bytes OR b.result IS DISTINCT FROM r.history#>'{value,result}') THEN
    RAISE EXCEPTION 'retry competing commits or original identity/history mismatch';
  END IF;
  IF has_table_privilege('kehila_auth_runtime','kehila_auth.pending_items','SELECT,INSERT,UPDATE,DELETE,TRUNCATE')
    OR has_function_privilege('kehila_auth_runtime','kehila_auth.item_write_fence(kehila_auth.authority,kehila.ident)','EXECUTE')
    OR EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_prepared_xacts WHERE database=current_database())
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) THEN
    RAISE EXCEPTION 'retry fixture privilege or runtime release mismatch';
  END IF;
END $$;
SELECT 'NATIVE_ITEM_RETRY_STATE_PASS';
