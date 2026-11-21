-- New backend after the closed-capability sequence commits.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET ROLE kehila_owner;
DO $$
DECLARE head kehila.current_work_selections;
BEGIN
  SELECT * INTO STRICT head FROM kehila.current_work_selections WHERE user_id='owner';
  IF head.event_ordinal<>11 OR head.selection_version<>6
     OR head.selected_project_id IS NOT NULL OR head.selected_item_id IS NOT NULL THEN
    RAISE EXCEPTION 'independent closed-selection head oracle failed';
  END IF;
  IF (SELECT count(*) FROM kehila.selection_events)<>11
     OR (SELECT count(*) FROM kehila.operations WHERE command_family='selection')<>11
     OR EXISTS(SELECT FROM kehila.operations WHERE command_family='selection' AND token IN(repeat('5',32),repeat('6',32))) THEN
    RAISE EXCEPTION 'independent closed-selection success/absence oracle failed';
  END IF;
  IF NOT EXISTS(SELECT FROM kehila.operations o JOIN kehila.selection_events e USING(operation_row_id)
      WHERE o.command_family='selection' AND o.token=repeat('1',32) AND o.payload_retired
        AND e.selected_project_id='p' AND e.selected_item_id='saved-item'
        AND NOT EXISTS(SELECT FROM kehila.operation_payloads p WHERE p.operation_row_id=o.operation_row_id)) THEN
    RAISE EXCEPTION 'retired original selection identity/scope was not preserved';
  END IF;
END;
$$;
RESET ROLE;
