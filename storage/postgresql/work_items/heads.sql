-- Fresh v3 operational heads. Requires records.sql and retention.sql.
-- Fixed write entry inserts a revision before advancing its operational head.
-- Serving grants belong to the coordinated installer capability packet.

-- Seed the operational witness for every ProjectCreate in the same transaction,
-- without changing its request, result or snapshot representation. The trusted
-- serving INSERT capability on projects does not grant raw head-table writes.
CREATE FUNCTION kehila.initialize_project_operational_head() RETURNS trigger
LANGUAGE plpgsql SECURITY DEFINER
SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  INSERT INTO kehila.project_operational_heads(project_id, state_version, operation_row_id)
    VALUES (NEW.project_id, 0, NEW.creation_operation_row_id);
  RETURN NULL;
END;
$$;
CREATE TRIGGER project_operational_initialization
  AFTER INSERT ON kehila.projects FOR EACH ROW
  EXECUTE FUNCTION kehila.initialize_project_operational_head();

CREATE FUNCTION kehila.protect_project_operational_head() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE
  project kehila.projects;
  previous record;
  successor record;
  previous_sequence kehila.u64;
  previous_estimated boolean;
BEGIN
  SELECT p.* INTO STRICT project FROM kehila.projects p WHERE p.project_id = NEW.project_id;
  IF TG_OP = 'INSERT' THEN
    IF NEW.state_version = 0 AND NEW.operation_row_id = project.creation_operation_row_id
       AND project.next_sequence = 1 AND NOT project.ever_estimated THEN
      RETURN NEW;
    END IF;
  ELSIF NEW.project_id = OLD.project_id AND NEW.state_version = OLD.state_version + 1 THEN
    IF OLD.state_version = 0 THEN
      IF OLD.operation_row_id <> project.creation_operation_row_id THEN
        RAISE EXCEPTION 'invalid initial operational head' USING ERRCODE = '23514',
          CONSTRAINT = 'project_operational_transition';
      END IF;
      previous_sequence := 1;
      previous_estimated := false;
    ELSE
      SELECT r.resulting_next_sequence, r.resulting_ever_estimated
        INTO STRICT previous FROM kehila.item_revisions r
        WHERE r.project_id = OLD.project_id AND r.project_state_version = OLD.state_version
          AND r.operation_row_id = OLD.operation_row_id;
      previous_sequence := previous.resulting_next_sequence;
      previous_estimated := previous.resulting_ever_estimated;
    END IF;
    SELECT r.previous_project_operation, r.resulting_next_sequence,
           r.resulting_ever_estimated, r.command_family, r.estimate_thousandths
      INTO STRICT successor FROM kehila.item_revisions r
      WHERE r.project_id = NEW.project_id AND r.project_state_version = NEW.state_version
        AND r.operation_row_id = NEW.operation_row_id;
    IF successor.previous_project_operation = OLD.operation_row_id
       AND successor.resulting_next_sequence = previous_sequence
           + (CASE successor.command_family WHEN 'item_create' THEN 1 ELSE 0 END)
       AND successor.resulting_ever_estimated =
           (previous_estimated OR successor.estimate_thousandths IS NOT NULL) THEN
      RETURN NEW;
    END IF;
  END IF;
  RAISE EXCEPTION 'invalid project operational transition'
    USING ERRCODE = '23514', CONSTRAINT = 'project_operational_transition',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'invalid project operational reference'
    USING ERRCODE = '23514', CONSTRAINT = 'project_operational_reference',
          SCHEMA = TG_TABLE_SCHEMA, TABLE = TG_TABLE_NAME;
END;
$$;
CREATE TRIGGER project_operational_transition
  BEFORE INSERT OR UPDATE ON kehila.project_operational_heads
  FOR EACH ROW EXECUTE FUNCTION kehila.protect_project_operational_head();
CREATE TRIGGER project_operational_no_delete
  BEFORE DELETE OR TRUNCATE ON kehila.project_operational_heads
  FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_item_history();

CREATE FUNCTION kehila.check_item_current_boundary() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE current_version kehila.u64; latest_version kehila.u64;
BEGIN
  SELECT i.current_version INTO STRICT current_version FROM kehila.items i
    WHERE i.project_id = NEW.project_id AND i.item_id = NEW.item_id;
  SELECT r.item_version INTO latest_version FROM kehila.item_revisions r
    WHERE r.project_id = NEW.project_id AND r.item_id = NEW.item_id
    ORDER BY r.item_version DESC LIMIT 1;
  IF latest_version IS NULL OR latest_version <> current_version THEN
    RAISE EXCEPTION 'item head is not its latest revision'
      USING ERRCODE = '23514', CONSTRAINT = 'item_current_boundary',
            SCHEMA = 'kehila', TABLE = 'items';
  END IF;
  RETURN NULL;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'invalid item current reference'
    USING ERRCODE = '23514', CONSTRAINT = 'item_current_boundary',
          SCHEMA = 'kehila', TABLE = 'items';
END;
$$;
CREATE CONSTRAINT TRIGGER item_current_boundary
  AFTER INSERT OR UPDATE ON kehila.items
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_item_current_boundary();
CREATE CONSTRAINT TRIGGER item_revision_current_boundary
  AFTER INSERT ON kehila.item_revisions
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_item_current_boundary();

CREATE FUNCTION kehila.check_project_current_boundary() RETURNS trigger
LANGUAGE plpgsql SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE
  project kehila.projects;
  head kehila.project_operational_heads;
  revision record;
  latest_version kehila.u64;
BEGIN
  SELECT p.* INTO STRICT project FROM kehila.projects p WHERE p.project_id = NEW.project_id;
  SELECT h.* INTO STRICT head FROM kehila.project_operational_heads h
    WHERE h.project_id = NEW.project_id;
  SELECT r.project_state_version INTO latest_version FROM kehila.item_revisions r
    WHERE r.project_id = NEW.project_id ORDER BY r.project_state_version DESC LIMIT 1;
  IF head.state_version = 0 THEN
    IF latest_version IS NOT NULL OR head.operation_row_id <> project.creation_operation_row_id
       OR project.next_sequence <> 1 OR project.ever_estimated THEN
      RAISE EXCEPTION 'invalid initial project boundary' USING ERRCODE = '23514',
        CONSTRAINT = 'project_current_boundary';
    END IF;
  ELSE
    SELECT r.resulting_next_sequence, r.resulting_ever_estimated
      INTO STRICT revision FROM kehila.item_revisions r
      WHERE r.project_id = head.project_id AND r.project_state_version = head.state_version
        AND r.operation_row_id = head.operation_row_id;
    IF latest_version IS DISTINCT FROM head.state_version
       OR project.next_sequence <> revision.resulting_next_sequence
       OR project.ever_estimated <> revision.resulting_ever_estimated THEN
      RAISE EXCEPTION 'invalid current project boundary' USING ERRCODE = '23514',
        CONSTRAINT = 'project_current_boundary';
    END IF;
  END IF;
  IF TG_TABLE_NAME = 'item_revisions' THEN
    IF NEW.project_state_version = 1 THEN
      IF NEW.previous_project_operation <> project.creation_operation_row_id THEN
        RAISE EXCEPTION 'invalid initial project predecessor' USING ERRCODE = '23514',
          CONSTRAINT = 'project_revision_predecessor';
      END IF;
    ELSIF NOT EXISTS (SELECT FROM kehila.item_revisions r
                      WHERE r.project_id = NEW.project_id
                        AND r.project_state_version = NEW.project_state_version - 1
                        AND r.operation_row_id = NEW.previous_project_operation) THEN
      RAISE EXCEPTION 'invalid project predecessor' USING ERRCODE = '23514',
        CONSTRAINT = 'project_revision_predecessor';
    END IF;
  END IF;
  RETURN NULL;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'invalid project current reference'
    USING ERRCODE = '23514', CONSTRAINT = 'project_current_boundary',
          SCHEMA = 'kehila', TABLE = 'project_operational_heads';
END;
$$;
CREATE CONSTRAINT TRIGGER project_current_boundary
  AFTER INSERT OR UPDATE ON kehila.projects
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_project_current_boundary();
CREATE CONSTRAINT TRIGGER project_head_current_boundary
  AFTER INSERT OR UPDATE ON kehila.project_operational_heads
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_project_current_boundary();
CREATE CONSTRAINT TRIGGER project_revision_current_boundary
  AFTER INSERT ON kehila.item_revisions
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
  EXECUTE FUNCTION kehila.check_project_current_boundary();

REVOKE ALL ON FUNCTION kehila.protect_project_operational_head(),
  kehila.initialize_project_operational_head(), kehila.check_item_current_boundary(),
  kehila.check_project_current_boundary() FROM PUBLIC;

-- No SELECT above acquires a row lock. Deferred closure must not acquire an
-- earlier lock class after pending/core/item acquisition. Fixed write entries
-- must protect the relevant project/item before accepting a prepared result.
-- JSON/relational agreement and family/completion closure are separate guards.
