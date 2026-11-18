-- Separate backend after the native HTTP/TLS selection fixture and runtime join.
-- This proves only the exact fixture state, not browser/IndexedDB behavior.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila.selection_events)<>12
     OR (SELECT count(*) FROM kehila.operations WHERE command_family='selection')<>12
     OR (SELECT count(*) FROM kehila.current_work_selections)<>1
     OR NOT EXISTS(SELECT FROM kehila.current_work_selections WHERE user_id='owner'
       AND selection_version=5 AND event_ordinal=12 AND selected_project_id IS NULL AND selected_item_id IS NULL)
     OR (SELECT array_agg(selection_version::numeric ORDER BY event_ordinal) FROM kehila.selection_events)
       IS DISTINCT FROM ARRAY[0,0,0,0,1,1,1,2,3,4,5,5]::numeric[]
     OR (SELECT array_agg(token::text ORDER BY event_ordinal) FROM kehila.selection_events)
       IS DISTINCT FROM ARRAY[lpad('10',32,'0'),lpad('11',32,'0'),lpad('12',32,'0'),repeat('0',32),repeat('a',32),repeat('1',32),repeat('b',32),
         repeat('c',32),repeat('d',32),repeat('e',32),repeat('f',32),repeat('7',32)]::text[]
     OR EXISTS(SELECT FROM kehila.selection_events e JOIN kehila.operations o USING(operation_row_id)
       WHERE o.actor_id<>'owner' OR o.target_user_id<>'owner' OR o.command_family<>'selection'
         OR o.target_kind<>'user' OR o.token<>e.token OR o.payload_retired)
     OR (SELECT count(*) FROM kehila.operation_payloads p JOIN kehila.operations o USING(operation_row_id)
       WHERE o.command_family='selection')<>12
     OR (SELECT count(*) FROM kehila.selection_events e
       JOIN kehila.operations o USING(operation_row_id)
       JOIN kehila.operation_payloads p USING(operation_row_id)
       WHERE o.actor_id=e.user_id AND o.target_user_id=e.user_id AND o.token=e.token
         AND o.request_codec_version=1 AND o.required_grant='selection' AND o.grant_policy_version=1
         AND o.request_sha256=sha256(p.request_bytes)
         AND p.request_bytes=kehila.selection_request_bytes(e.user_id,e.token,e.before_version,
           e.selected_project_id,e.selected_item_id)
         AND p.result_codec_version=1 AND e.history_codec_version=1
         AND p.result=jsonb_build_object('codec_version',1,'kind','selection_result','value',
           kehila.selection_state_value(e.user_id,e.selection_version,e.selected_project_id,e.selected_item_id))
         AND e.history #>'{value,result}'=p.result
         AND e.history #>'{value,before}'=kehila.selection_state_value(e.user_id,e.before_version,e.before_project_id,e.before_item_id)
         AND e.history #>>'{value,operation_id}'=e.token
         AND e.history #>>'{value,ordinal}'=e.event_ordinal::numeric(20,0)::text)<>12
     OR EXISTS(SELECT FROM kehila.operations WHERE command_family='selection'
       AND token IN(repeat('2',32),repeat('6',32),repeat('8',32),repeat('9',32)))
     OR (SELECT count(*) FROM kehila.items)<>2
     OR (SELECT count(*) FROM kehila.item_revisions)<>2
     OR EXISTS(SELECT FROM kehila.items WHERE current_version<>1)
     OR (SELECT count(*) FROM kehila.project_operational_heads WHERE state_version=2)<>1
     OR NOT EXISTS(SELECT FROM kehila.selection_events WHERE token=repeat('a',32)
       AND selection_version=1 AND selected_item_id='read-fixture-1')
     OR NOT EXISTS(SELECT FROM kehila.selection_events WHERE token=repeat('c',32)
       AND selection_version=2 AND selected_item_id='read-fixture-2')
     OR NOT EXISTS(SELECT FROM kehila.selection_events WHERE token=repeat('e',32)
       AND selection_version=4 AND selected_item_id='read-fixture-1')
  THEN
    RAISE EXCEPTION 'native selection committed-state mismatch';
  END IF;
END;
$$;
SELECT 'PASS: native selection original results, no-op history and final empty head';
