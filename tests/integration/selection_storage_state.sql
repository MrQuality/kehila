-- Separate backend observation of committed structural fixture state.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET ROLE kehila_owner;
DO $$
DECLARE head record; events bigint; cores bigint; payloads bigint;
BEGIN
  SELECT * INTO STRICT head FROM kehila.current_work_selections WHERE user_id='owner';
  IF head.selection_version<>3 OR head.event_ordinal<>6
     OR head.selected_project_id IS NOT NULL OR head.selected_item_id IS NOT NULL THEN
    RAISE EXCEPTION 'independent selection head oracle failed';
  END IF;
  SELECT count(*) INTO events FROM kehila.selection_events e
    JOIN kehila.operations o ON o.operation_row_id=e.operation_row_id
      AND o.actor_id=e.actor_id AND o.target_user_id=e.user_id
      AND o.command_family='selection' AND o.target_kind='user' AND o.token=e.token
    WHERE e.user_id='owner'
      AND e.history #>>'{value,ordinal}'=e.event_ordinal::numeric(20,0)::text;
  SELECT count(*) INTO cores FROM kehila.operations WHERE command_family='selection';
  SELECT count(*) INTO payloads FROM kehila.operation_payloads p
    JOIN kehila.operations o USING(operation_row_id)
    JOIN kehila.selection_events e USING(operation_row_id) WHERE o.command_family='selection'
      AND sha256(p.request_bytes)=o.request_sha256
      AND p.result=e.history #>'{value,result}';
  IF events<>6 OR cores<>6 OR payloads<>6 OR
     (SELECT array_agg(selection_version::numeric ORDER BY event_ordinal) FROM kehila.selection_events WHERE user_id='owner')
       IS DISTINCT FROM ARRAY[0,0,1,1,2,3]::numeric[] THEN
    RAISE EXCEPTION 'independent selection event/core/payload oracle failed: %/%/%',events,cores,payloads;
  END IF;
END;
$$;
RESET ROLE;
