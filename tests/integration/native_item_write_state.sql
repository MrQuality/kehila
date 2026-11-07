-- Independent connection after the isolated owner-adapter write fixture.
-- Operation identity values may have gaps after the deliberate failed save.
DO $$ BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>1
    OR (SELECT count(*) FROM kehila.items)<>3
    OR (SELECT count(*) FROM kehila.item_revisions)<>5
    OR (SELECT count(*) FROM kehila.operations)<>6
    OR (SELECT count(*) FROM kehila.operation_payloads)<>6
    OR (SELECT count(*) FROM kehila_auth.pending_items)<>4
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='completed')<>3
    OR (SELECT count(*) FROM kehila_auth.pending_items WHERE state='prepared')<>1
    -- Uses are first-use witnesses per project/type/field, not revision counts.
    OR (SELECT count(*) FROM kehila.item_field_uses)<>1
    OR NOT EXISTS(SELECT FROM kehila.projects p JOIN kehila.project_operational_heads h USING(project_id)
        JOIN kehila.item_revisions r ON r.project_id=p.project_id AND r.operation_row_id=h.operation_row_id
      WHERE p.name='Item write fixture' AND p.next_sequence=4 AND p.ever_estimated
        AND h.state_version=5 AND r.item_id='write-unknown' AND r.project_state_version=5)
    OR NOT EXISTS(SELECT FROM kehila.items WHERE item_id='write-task' AND current_version=3 AND issued_sequence=2 AND issued_prefix='IWR')
    OR NOT EXISTS(SELECT FROM kehila.items WHERE item_id='write-milestone' AND current_version=1 AND issued_sequence=1 AND issued_prefix='IWR')
    OR NOT EXISTS(SELECT FROM kehila.items WHERE item_id='write-unknown' AND current_version=1 AND issued_sequence=3 AND issued_prefix='IWR')
    OR EXISTS(SELECT FROM kehila.items WHERE item_id='write-rejected')
    OR EXISTS(SELECT FROM kehila.operations WHERE target_item_id='write-rejected') THEN
    RAISE EXCEPTION 'item write fixture cardinality or identity mismatch';
  END IF;
  IF NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_id='write-task' AND r.item_version=1
        AND r.estimate_thousandths=0 AND r.project_state_version=2 AND r.token=repeat('a',32)
        AND r.history#>>'{value,result,value,data,values,0,value,value}'='  exact task  ')
    OR NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_id='write-task' AND r.item_version=2
        AND r.previous_item_version=1 AND r.estimate_thousandths IS NULL AND r.project_state_version=3
        AND r.token=repeat('c',32) AND r.resulting_ever_estimated
        AND r.history#>'{value,result,value,data,values}'='[]'::jsonb)
    OR NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_id='write-task' AND r.item_version=3
        AND r.previous_item_version=2 AND r.estimate_thousandths=999999999 AND r.project_state_version=4
        AND r.token=repeat('e',32) AND r.resulting_ever_estimated
        AND convert_to(r.history#>>'{value,result,value,data,values,0,value,value}','UTF8')
          =decode('202065786163740d0ac3a9207469746c652020','hex'))
    OR NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_id='write-milestone' AND r.project_state_version=1
        AND r.token=repeat('b',32) AND r.estimate_thousandths IS NULL
        AND r.history#>'{value,result,value,data,values}'='[]'::jsonb)
    OR NOT EXISTS(SELECT FROM kehila.item_revisions r WHERE r.item_id='write-unknown' AND r.project_state_version=5
        AND r.token=repeat('1',32) AND r.estimate_thousandths IS NULL
        AND r.history#>'{value,result,value,data,values}'='[]'::jsonb)
    OR NOT EXISTS(SELECT FROM kehila.item_field_uses u JOIN kehila.item_revisions r
        ON r.project_id=u.project_id AND r.item_id=u.first_item_id AND r.item_version=u.first_item_version
        AND r.operation_row_id=u.first_operation_row_id
      WHERE u.first_item_id='write-task' AND u.first_item_version=1
        AND u.field_id=(r.history#>>'{value,result,value,data,values,0,id}')::kehila.ident) THEN
    RAISE EXCEPTION 'item write fixture configured value or first-use mismatch';
  END IF;
  IF EXISTS(SELECT FROM kehila.item_revisions r JOIN kehila.operations o USING(operation_row_id)
        JOIN kehila.operation_payloads b USING(operation_row_id)
      WHERE o.actor_id<>r.actor_id OR o.actor_id<>'owner' OR o.command_family<>r.command_family
        OR o.target_project_id<>r.project_id OR o.target_item_id<>r.item_id OR o.token<>r.token
        OR o.request_codec_version<>1 OR sha256(b.request_bytes)<>o.request_sha256
        OR r.history#>>'{value,result,value,project,name}' IS DISTINCT FROM 'Item write fixture')
    OR EXISTS(SELECT FROM kehila.item_revisions r JOIN kehila.projects p USING(project_id)
        LEFT JOIN kehila.item_revisions previous ON previous.project_id=r.project_id
          AND previous.project_state_version=r.project_state_version-1
      WHERE r.previous_project_operation IS DISTINCT FROM
        CASE WHEN r.project_state_version=1 THEN p.creation_operation_row_id ELSE previous.operation_row_id END)
    OR EXISTS(SELECT FROM kehila.items i JOIN kehila_auth.pending_items p ON p.request_id=i.creation_request_id
        JOIN kehila.item_revisions r ON r.project_id=i.project_id AND r.item_id=i.item_id AND r.item_version=1
      WHERE p.state<>'completed' OR p.target_item_id<>i.item_id OR p.project_id<>i.project_id
        OR p.operation_row_id<>r.operation_row_id OR i.creation_operation_row_id<>r.operation_row_id
        OR p.token<>r.token OR p.actor_id<>r.actor_id) THEN
    RAISE EXCEPTION 'item write fixture attribution or history linkage mismatch';
  END IF;
  IF NOT EXISTS(SELECT FROM kehila_auth.pending_items p JOIN kehila.configuration_revisions c
        ON c.project_id=p.project_id AND c.revision=1
      WHERE p.target_item_id='write-rejected' AND p.intent_handle=repeat('f',32) AND p.token=repeat('f',32)
        AND p.state='prepared' AND p.operation_row_id IS NULL AND p.request_codec_version=1
        AND sha256(p.request_bytes)=p.request_sha256
        AND convert_from(p.request_bytes,'UTF8')::jsonb=jsonb_build_object(
          'codec_version',1,'kind','item_create_request','value',jsonb_build_object(
            'project_id',p.project_id::text,'operation_id',repeat('f',32),'item_id','write-rejected',
            'type_id',c.configuration_snapshot#>>'{value,types,0,id}',
            'workflow_id',NULL,'expected_configuration_revision','1','estimate',NULL,
            'values',jsonb_build_array(jsonb_build_object(
              'id',c.configuration_snapshot#>>'{value,types,0,title_field_id}',
              'value',jsonb_build_object('kind','text','value','valid provisional title')))))) THEN
    RAISE EXCEPTION 'rolled-back intent did not retain exact request binding';
  END IF;
  IF EXISTS(SELECT FROM pg_stat_activity WHERE datname=current_database() AND usename='kehila_auth_runtime_login')
    OR EXISTS(SELECT FROM pg_prepared_xacts WHERE database=current_database())
    OR EXISTS(SELECT FROM pg_locks WHERE locktype='advisory'
      AND database=(SELECT oid FROM pg_database WHERE datname=current_database())
      AND classid=1262831689::oid AND objid=1::oid AND objsubid=2) THEN
    RAISE EXCEPTION 'item write fixture retains runtime ownership';
  END IF;
END $$;
SELECT 'NATIVE_ITEM_WRITE_STATE_PASS';
