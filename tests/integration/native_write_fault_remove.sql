SET ROLE kehila_owner;
DO $$ DECLARE target text; BEGIN
  FOREACH target IN ARRAY ARRAY['operations','operation_payloads','projects','configuration_revisions',
    'project_grants','project_grant_events','statuses','workflows','workflow_statuses','workflow_phase_changes',
    'work_item_types','type_workflows','fields','project_operational_heads'] LOOP
    EXECUTE format('DROP TRIGGER native_write_fault ON kehila.%I',target);
  END LOOP;
END $$;
DROP TRIGGER native_write_fault ON kehila_auth.pending_projects;
DROP TRIGGER native_write_fault ON kehila_auth.creation_intents;
DROP TRIGGER native_write_finish ON kehila_auth.creation_intents;
DROP FUNCTION kehila_native_fixture.write_count(),kehila_native_fixture.write_finish();
DROP SEQUENCE kehila_native_fixture.write_ordinal;
DROP TABLE kehila_native_fixture.write_control;
RESET ROLE;
