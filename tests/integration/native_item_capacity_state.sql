-- Independent durable-state witness after default and synthetic input controls.
-- Synthetic profiles are not alternative canonical product initialization seeds.
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>3
    OR (SELECT count(*) FROM kehila.fields)<>136
    OR (SELECT count(*) FROM kehila.items)<>3
    OR (SELECT count(*) FROM kehila.item_revisions)<>6
    OR (SELECT count(*) FROM kehila.operations)<>9
    OR (SELECT count(*) FROM kehila.operation_payloads)<>9
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='project_create')<>3
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='item_create')<>3
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='item_edit')<>3
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>3
    OR EXISTS(SELECT FROM kehila_auth.pending_items WHERE state<>'completed')
    OR EXISTS(SELECT FROM kehila.items WHERE current_version<>2 OR issued_sequence<>1)
    OR EXISTS(SELECT FROM kehila.projects WHERE next_sequence<>2 OR ever_estimated)
    OR EXISTS(SELECT FROM kehila.project_operational_heads WHERE state_version<>2) THEN
    RAISE EXCEPTION 'capacity input control cardinality or state mismatch';
  END IF;
  IF EXISTS(SELECT FROM kehila.item_revisions r
      JOIN kehila.projects p USING(project_id)
      JOIN kehila.operations o USING(operation_row_id)
      JOIN kehila.operation_payloads b USING(operation_row_id)
      WHERE o.actor_id<>r.actor_id OR o.token<>r.token
        OR (r.item_version=2 AND r.token<>lpad(to_hex(CASE p.current_prefix
          WHEN 'CPC' THEN 100 WHEN 'CPS' THEN 101 WHEN 'CPL' THEN 102 ELSE -1 END),32,'0'))
        OR o.target_item_id<>r.item_id OR o.target_project_id<>r.project_id
        OR o.request_sha256<>sha256(b.request_bytes)
        OR b.result IS DISTINCT FROM r.history#>'{value,result}'
        OR jsonb_array_length(b.result#>'{value,data,values}')<>
          CASE p.current_prefix WHEN 'CPC' THEN 2 WHEN 'CPS' THEN 4 WHEN 'CPL' THEN 128 ELSE -1 END
        OR EXISTS(SELECT FROM jsonb_array_elements(b.result#>'{value,data,values}') value
          WHERE value#>>'{value,kind}'<>'text'
            OR value#>>'{value,value}' IS DISTINCT FROM repeat(
              CASE r.item_version WHEN 1 THEN 'a' WHEN 2 THEN 'b' ELSE '?' END,
              CASE p.current_prefix WHEN 'CPL' THEN 1024 ELSE 64 END))) THEN
    RAISE EXCEPTION 'capacity input control exact identity, payload or history mismatch';
  END IF;
  IF (SELECT count(*) FROM kehila_auth.pending_items p
        JOIN kehila.items i ON i.creation_request_id=p.request_id)<>3
    OR EXISTS(SELECT FROM kehila_auth.pending_items p
      JOIN kehila.items i ON i.creation_request_id=p.request_id
      JOIN kehila.operations o ON o.operation_row_id=i.creation_operation_row_id
      JOIN kehila.operation_payloads b ON b.operation_row_id=o.operation_row_id
      WHERE p.project_id<>i.project_id OR p.target_item_id<>i.item_id
        OR p.operation_row_id<>o.operation_row_id
        OR p.token<>o.token OR p.request_codec_version<>1 OR p.request_bytes IS NULL
        OR p.request_sha256<>o.request_sha256 OR p.request_sha256<>sha256(p.request_bytes)
        OR p.request_bytes<>b.request_bytes) THEN
    RAISE EXCEPTION 'capacity input control original creation identity or bytes mismatch';
  END IF;
  IF EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database()
        AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_prepared_xacts WHERE database=current_database())
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2)
    OR has_table_privilege('kehila_auth_runtime','kehila_auth.pending_items','SELECT,INSERT,UPDATE,DELETE,TRUNCATE') THEN
    RAISE EXCEPTION 'capacity input control runtime release or privilege mismatch';
  END IF;
END $$;
SELECT 'NATIVE_ITEM_CAPACITY_STATE_PASS';
