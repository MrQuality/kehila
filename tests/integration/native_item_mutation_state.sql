-- Independent committed-state witness after the serving HTTP/TLS journey.
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>1 OR (SELECT count(*) FROM kehila.items)<>2
    OR (SELECT count(*) FROM kehila.item_revisions)<>4 OR (SELECT count(*) FROM kehila.operations)<>5
    OR (SELECT count(*) FROM kehila.operation_payloads)<>3
    OR (SELECT count(*) FROM kehila.operations WHERE payload_retired)<>2
    OR (SELECT count(*) FROM kehila.operations WHERE payload_retired AND command_family='item_edit'
      AND token IN (repeat('c',32),repeat('d',32)))<>2
    OR EXISTS(SELECT FROM kehila.operations o JOIN kehila.operation_payloads p USING(operation_row_id)
      WHERE o.payload_retired)
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>3
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='completed')<>2
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='cancelled')<>1
    OR EXISTS(SELECT FROM kehila_auth.pending_items p WHERE p.state='cancelled'
      AND (p.operation_row_id IS NOT NULL OR p.request_bytes IS NOT NULL
        OR EXISTS(SELECT FROM kehila.items i WHERE i.item_id=p.target_item_id)
        OR EXISTS(SELECT FROM kehila.operations o WHERE o.command_family='item_create'
          AND o.target_project_id=p.project_id AND o.token=p.token)))
    OR (SELECT count(*) FROM kehila.item_field_uses)<>1
    OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
      JOIN kehila.item_revisions r ON r.project_id=p.project_id AND r.operation_row_id=h.operation_row_id
      WHERE p.name='Item mutation journey' AND p.current_prefix='IMJ' AND p.next_sequence=3
        AND p.ever_estimated AND h.state_version=4 AND r.item_version=3 AND r.token=repeat('d',32))
    OR (SELECT count(*) FROM kehila.items WHERE current_version=3 AND issued_sequence=1 AND issued_prefix='IMJ')<>1
    OR (SELECT count(*) FROM kehila.items WHERE current_version=1 AND issued_sequence=2 AND issued_prefix='IMJ')<>1 THEN
    RAISE EXCEPTION 'item serving mutation cardinality or state mismatch';
  END IF;
  IF NOT EXISTS(SELECT FROM kehila.items i JOIN kehila.item_revisions r USING(project_id,item_id)
      WHERE i.issued_sequence=1 AND r.item_version=1 AND r.estimate_thousandths=0
        AND convert_to(r.history#>>'{value,result,value,data,values,0,value,value}','UTF8')
          =decode('202065786163740d0ac3a9207469746c652020','hex'))
    OR NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_version=2
      AND r.token=repeat('c',32) AND r.estimate_thousandths=999999999
      AND convert_to(r.history#>>'{value,result,value,data,values,0,value,value}','UTF8')
        =decode('202065786163740d0ac3a9207469746c652020','hex'))
    OR NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_version=3
      AND r.token=repeat('d',32) AND r.estimate_thousandths IS NULL
      AND r.history#>'{value,result,value,data,values}'='[]'::jsonb AND r.resulting_ever_estimated)
    OR EXISTS(SELECT FROM kehila.operations WHERE command_family='item_edit' AND token IN (repeat('e',32),repeat('f',32)))
    OR EXISTS(SELECT FROM kehila.item_revisions r JOIN kehila.operations o USING(operation_row_id)
      LEFT JOIN kehila.operation_payloads p USING(operation_row_id)
      WHERE o.actor_id<>r.actor_id OR o.actor_id<>'owner' OR o.command_family<>r.command_family
        OR o.target_project_id<>r.project_id OR o.target_item_id<>r.item_id OR o.token<>r.token
        OR o.request_codec_version<>1
        OR (o.payload_retired AND p.operation_row_id IS NOT NULL)
        OR (NOT o.payload_retired AND (p.operation_row_id IS NULL
          OR sha256(p.request_bytes)<>o.request_sha256
          OR p.result IS DISTINCT FROM r.history#>'{value,result}')))
    OR EXISTS(SELECT FROM kehila.items i JOIN kehila_auth.pending_items p ON p.request_id=i.creation_request_id
      JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=1
      WHERE p.state<>'completed' OR p.project_id<>i.project_id OR p.target_item_id<>i.item_id
        OR p.operation_row_id<>r.operation_row_id OR i.creation_operation_row_id<>r.operation_row_id
        OR p.actor_id<>r.actor_id OR p.token<>r.token) THEN
    RAISE EXCEPTION 'item serving mutation history or replay identity mismatch';
  END IF;
  IF to_regclass('kehila_native_fixture.clock') IS NOT NULL
    OR to_regclass('kehila_native_fixture.original_clock') IS NOT NULL
    OR has_table_privilege('kehila_auth_runtime','kehila_auth.pending_items','SELECT,INSERT,UPDATE,DELETE,TRUNCATE')
    OR has_function_privilege('kehila_auth_runtime','kehila_auth.item_write_fence(kehila_auth.authority,kehila.ident)','EXECUTE')
    OR EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_prepared_xacts WHERE database=current_database())
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) THEN
    RAISE EXCEPTION 'item serving mutation privilege or runtime release mismatch';
  END IF;
END $$;
SELECT 'NATIVE_ITEM_MUTATION_STATE_PASS';
