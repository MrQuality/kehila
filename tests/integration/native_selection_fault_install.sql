-- Disposable native fixture only. Install atomically under its privileged owner.
BEGIN;
SET LOCAL ROLE kehila_owner;
DO $$ BEGIN
  IF (SELECT value FROM kehila_native_fixture.marker) IS DISTINCT FROM 'disposable native fixture' THEN
    RAISE EXCEPTION 'explicit disposable native fixture required';
  END IF;
END; $$;
CREATE TABLE kehila_native_fixture.selection_fault_control (
  singleton boolean PRIMARY KEY CHECK(singleton),
  token kehila.operation_token NOT NULL
);
INSERT INTO kehila_native_fixture.selection_fault_control VALUES(true,repeat('6',32));
REVOKE ALL ON kehila_native_fixture.selection_fault_control FROM PUBLIC;
CREATE FUNCTION kehila_native_fixture.fail_selection_head() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, kehila, kehila_native_fixture, pg_temp AS $$
BEGIN
  IF EXISTS(SELECT FROM kehila.selection_events e
    JOIN kehila_native_fixture.selection_fault_control c ON c.token=e.token
    WHERE e.user_id=NEW.user_id AND e.event_ordinal=NEW.event_ordinal) THEN
    RAISE EXCEPTION 'injected selection failure after head write' USING ERRCODE='PZ001';
  END IF;
  RETURN NEW;
END;
$$;
REVOKE ALL ON FUNCTION kehila_native_fixture.fail_selection_head() FROM PUBLIC;
CREATE TRIGGER selection_injected_head_failure AFTER INSERT OR UPDATE
  ON kehila.current_work_selections FOR EACH ROW
  EXECUTE FUNCTION kehila_native_fixture.fail_selection_head();
COMMIT;
