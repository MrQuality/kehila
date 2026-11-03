-- Disposable test fixture only. Never include this in a product installer.
SET ROLE kehila_owner;
DO $$ BEGIN
  IF (SELECT value FROM kehila_native_fixture.marker) <> 'disposable native fixture' THEN
    RAISE EXCEPTION 'explicit disposable native fixture required';
  END IF;
END $$;
CREATE TABLE kehila_native_fixture.write_control (
  singleton boolean PRIMARY KEY CHECK(singleton), request_id bigint NOT NULL,
  target text NOT NULL, fail_at bigint, finish_failure boolean NOT NULL, trace boolean NOT NULL);
CREATE SEQUENCE kehila_native_fixture.write_ordinal CACHE 1 NO CYCLE;
CREATE FUNCTION kehila_native_fixture.write_count() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE control kehila_native_fixture.write_control; data jsonb; target text; ordinal bigint;
BEGIN
  SELECT * INTO STRICT control FROM kehila_native_fixture.write_control WHERE singleton;
  data := to_jsonb(NEW);
  IF TG_TABLE_SCHEMA='kehila_auth' THEN
    IF (data->>'request_id')::bigint <> control.request_id THEN RETURN NEW; END IF;
    IF TG_TABLE_NAME='pending_projects' THEN
      IF OLD.state<>'prepared' OR NEW.state<>'completed' THEN RETURN NEW; END IF;
    END IF;
    target := data->>'target_project_id';
  ELSIF TG_TABLE_NAME='operations' THEN
    target := data->>'target_project_id';
  ELSIF TG_TABLE_NAME='operation_payloads' THEN
    SELECT o.target_project_id INTO STRICT target FROM kehila.operations o
      WHERE o.operation_row_id=(data->>'operation_row_id')::bigint;
  ELSE
    target := data->>'project_id';
  END IF;
  IF target IS DISTINCT FROM control.target THEN RETURN NEW; END IF;
  ordinal := nextval('kehila_native_fixture.write_ordinal'::regclass);
  IF control.trace THEN
    RAISE WARNING 'NATIVE_WRITE % % %',ordinal,TG_TABLE_SCHEMA||'.'||TG_TABLE_NAME,TG_OP;
  END IF;
  IF ordinal=control.fail_at THEN
    RAISE EXCEPTION 'owned native write fault' USING ERRCODE='PZ001';
  END IF;
  RETURN NEW;
END $$;
CREATE FUNCTION kehila_native_fixture.write_finish() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER SET search_path=pg_catalog,pg_temp AS $$
DECLARE control kehila_native_fixture.write_control;
BEGIN
  SELECT * INTO STRICT control FROM kehila_native_fixture.write_control WHERE singleton;
  IF NEW.request_id=control.request_id AND NEW.target_project_id=control.target AND control.finish_failure THEN
    RAISE EXCEPTION 'owned native precommit fault' USING ERRCODE='PZ001';
  END IF;
  RETURN NULL;
END $$;
DO $$ DECLARE target text; BEGIN
  FOREACH target IN ARRAY ARRAY['operations','operation_payloads','projects','configuration_revisions',
    'project_grants','project_grant_events','statuses','workflows','workflow_statuses','workflow_phase_changes',
    'work_item_types','type_workflows','fields'] LOOP
    EXECUTE format('CREATE TRIGGER native_write_fault BEFORE INSERT ON kehila.%I FOR EACH ROW EXECUTE FUNCTION kehila_native_fixture.write_count()',target);
  END LOOP;
END $$;
CREATE TRIGGER native_write_fault BEFORE UPDATE ON kehila_auth.pending_projects
  FOR EACH ROW EXECUTE FUNCTION kehila_native_fixture.write_count();
CREATE TRIGGER native_write_fault BEFORE INSERT ON kehila_auth.creation_intents
  FOR EACH ROW EXECUTE FUNCTION kehila_native_fixture.write_count();
CREATE CONSTRAINT TRIGGER native_write_finish AFTER INSERT ON kehila_auth.creation_intents
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION kehila_native_fixture.write_finish();
REVOKE ALL ON kehila_native_fixture.write_control FROM PUBLIC;
REVOKE ALL ON SEQUENCE kehila_native_fixture.write_ordinal FROM PUBLIC;
REVOKE ALL ON FUNCTION kehila_native_fixture.write_count(),kehila_native_fixture.write_finish() FROM PUBLIC;
RESET ROLE;
