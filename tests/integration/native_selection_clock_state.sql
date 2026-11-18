-- Separate connection after the bounded payload-wait experiment and runtime join.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
DO $$
BEGIN
  IF (SELECT count(*) FROM kehila.projects)<>1
    OR (SELECT count(*) FROM kehila.items)<>1
    OR (SELECT count(*) FROM kehila.operations)<>3
    OR (SELECT count(*) FROM kehila.operations WHERE command_family='selection')<>1
    OR (SELECT count(*) FROM kehila.selection_events)<>1
    OR NOT EXISTS(SELECT FROM kehila.current_work_selections WHERE user_id='owner'
      AND event_ordinal=1 AND selection_version=1 AND selected_item_id='read-fixture-1')
    OR NOT EXISTS(SELECT FROM kehila.selection_events WHERE user_id='owner' AND event_ordinal=1
      AND token=repeat('0',32) AND before_version=0 AND selection_version=1)
    OR NOT EXISTS(SELECT FROM kehila.selection_events e JOIN kehila.operations o USING(operation_row_id)
      WHERE e.user_id='owner' AND e.actor_id='owner' AND e.token=repeat('0',32)
        AND o.actor_id=e.actor_id AND o.token=e.token AND o.command_family='selection'
        AND o.target_kind='user' AND o.target_user_id=e.user_id AND o.payload_retired
        AND o.request_sha256=sha256(kehila.selection_request_bytes(e.user_id,e.token,
          e.before_version,e.selected_project_id,e.selected_item_id))
        AND e.history #>>'{value,ordinal}'='1' AND e.history #>>'{value,operation_id}'=e.token
        AND e.history #>'{value,before}'=kehila.selection_state_value('owner',0,NULL,NULL)
        AND e.history #>'{value,result}'=jsonb_build_object('codec_version',1,'kind','selection_result',
          'value',kehila.selection_state_value('owner',1,e.selected_project_id,e.selected_item_id)))
    OR EXISTS(SELECT FROM kehila.operation_payloads p JOIN kehila.operations o USING(operation_row_id)
      WHERE o.command_family='selection')
    OR EXISTS(SELECT FROM kehila.project_grants WHERE NOT active)
    OR NOT EXISTS(SELECT FROM kehila.actors WHERE actor_id='owner' AND can_create_project)
    OR to_regclass('kehila_native_fixture.clock') IS NOT NULL
    OR to_regclass('kehila_native_fixture.original_clock') IS NOT NULL
    OR (SELECT prosecdef FROM pg_proc WHERE oid='kehila_auth.trusted_millis()'::regprocedure)
  THEN
    RAISE EXCEPTION 'selection clock experiment final state mismatch';
  END IF;
END;
$$;
SELECT 'PASS: selection fixture state preserved and clock restored';
