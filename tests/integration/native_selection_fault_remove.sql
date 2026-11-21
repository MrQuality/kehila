BEGIN;
SET LOCAL ROLE kehila_owner;
DROP TRIGGER selection_injected_head_failure ON kehila.current_work_selections;
DROP FUNCTION kehila_native_fixture.fail_selection_head();
DROP TABLE kehila_native_fixture.selection_fault_control;
COMMIT;
