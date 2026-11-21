CREATE FUNCTION kehila.selection_state_value(subject kehila.ident,version kehila.u64,
  project kehila.ident,item kehila.ident) RETURNS jsonb
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, kehila, pg_temp AS $$
  SELECT jsonb_build_object('user_id',subject,'version',version::numeric(20,0)::text,
    'requested',CASE WHEN project IS NULL THEN 'null'::jsonb
      ELSE jsonb_build_object('project_id',project,'item_id',item) END);
$$;

-- Network-order u64 without a signed-bigint cast at the high-bit boundary.
CREATE FUNCTION kehila.selection_u64_bytes(value kehila.u64) RETURNS bytea
LANGUAGE sql IMMUTABLE STRICT SET search_path = pg_catalog, kehila, pg_temp AS $$
  SELECT int4send((CASE WHEN div(value,4294967296)>=2147483648
      THEN div(value,4294967296)-4294967296 ELSE div(value,4294967296) END)::integer)
    ||int4send((CASE WHEN mod(value,4294967296)>=2147483648
      THEN mod(value,4294967296)-4294967296 ELSE mod(value,4294967296) END)::integer);
$$;
CREATE FUNCTION kehila.selection_request_bytes(subject kehila.ident,token kehila.operation_token,
  expected kehila.u64,project kehila.ident,item kehila.ident) RETURNS bytea
LANGUAGE sql IMMUTABLE SET search_path = pg_catalog, kehila, pg_temp AS $$
  SELECT convert_to('kehila','UTF8')||decode('00','hex')||convert_to('selection','UTF8')||decode('0000000001','hex')
    ||int4send(octet_length(subject))||convert_to(subject,'UTF8')
    ||int4send(octet_length(token))||convert_to(token,'UTF8')||kehila.selection_u64_bytes(expected)
    ||CASE WHEN project IS NULL THEN decode('00','hex') ELSE decode('01','hex')
      ||int4send(octet_length(project))||convert_to(project,'UTF8')
      ||int4send(octet_length(item))||convert_to(item,'UTF8') END;
$$;

CREATE FUNCTION kehila.protect_selection_history() RETURNS trigger LANGUAGE plpgsql
SET search_path = pg_catalog, kehila, pg_temp AS $$
BEGIN
  RAISE EXCEPTION 'selection history cannot be changed or removed'
    USING ERRCODE='23514',CONSTRAINT='selection_history_immutable';
END;
$$;
CREATE TRIGGER selection_event_immutable BEFORE UPDATE OR DELETE OR TRUNCATE
  ON kehila.selection_events FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_selection_history();
CREATE TRIGGER selection_current_no_delete BEFORE DELETE OR TRUNCATE
  ON kehila.current_work_selections FOR EACH STATEMENT EXECUTE FUNCTION kehila.protect_selection_history();

CREATE FUNCTION kehila.check_selection_event() RETURNS trigger LANGUAGE plpgsql
SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE predecessor kehila.selection_events; expected_result jsonb; expected_history jsonb;
BEGIN
  IF NEW.event_ordinal=1 THEN
    IF NEW.before_version<>0 OR NEW.before_project_id IS NOT NULL OR NEW.before_item_id IS NOT NULL THEN
      RAISE EXCEPTION 'invalid initial selection predecessor'
        USING ERRCODE='23514',CONSTRAINT='selection_event_predecessor';
    END IF;
  ELSE
    SELECT * INTO STRICT predecessor FROM kehila.selection_events
      WHERE user_id=NEW.user_id AND event_ordinal=NEW.previous_event_ordinal;
    IF predecessor.selection_version<>NEW.before_version
       OR (predecessor.selected_project_id,predecessor.selected_item_id)
         IS DISTINCT FROM (NEW.before_project_id,NEW.before_item_id) THEN
      RAISE EXCEPTION 'selection predecessor disagrees'
        USING ERRCODE='23514',CONSTRAINT='selection_event_predecessor';
    END IF;
  END IF;
  expected_result:=jsonb_build_object('codec_version',1,'kind','selection_result','value',
    kehila.selection_state_value(NEW.user_id,NEW.selection_version,NEW.selected_project_id,NEW.selected_item_id));
  expected_history:=jsonb_build_object('codec_version',1,'kind','selection_event','value',
    jsonb_build_object('ordinal',NEW.event_ordinal::numeric(20,0)::text,'operation_id',NEW.token,
      'before',kehila.selection_state_value(NEW.user_id,NEW.before_version,NEW.before_project_id,NEW.before_item_id),
      'result',expected_result));
  IF NEW.history_codec_version<>1 OR NEW.history IS DISTINCT FROM expected_history THEN
    RAISE EXCEPTION 'selection relational and stored history disagree'
      USING ERRCODE='23514',CONSTRAINT='selection_history_agreement';
  END IF;
  RETURN NEW;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'invalid selection predecessor'
    USING ERRCODE='23514',CONSTRAINT='selection_event_predecessor';
END;
$$;
CREATE TRIGGER selection_event_history BEFORE INSERT ON kehila.selection_events
  FOR EACH ROW EXECUTE FUNCTION kehila.check_selection_event();

CREATE FUNCTION kehila.protect_selection_head() RETURNS trigger LANGUAGE plpgsql
SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE successor kehila.selection_events;
BEGIN
  SELECT * INTO STRICT successor FROM kehila.selection_events
    WHERE user_id=NEW.user_id AND event_ordinal=NEW.event_ordinal;
  IF TG_OP='INSERT' THEN
    -- PostgreSQL invokes BEFORE INSERT before choosing ON CONFLICT UPDATE.
    -- An existing subject may therefore propose a successor here; its actual
    -- before-state is checked by the subsequent UPDATE trigger. A plain
    -- duplicate INSERT still cannot bypass the primary key.
    IF NEW.event_ordinal<>1 AND NOT EXISTS
      (SELECT FROM kehila.current_work_selections WHERE user_id=NEW.user_id) THEN
      RAISE no_data_found;
    END IF;
  ELSE
    IF NEW.user_id<>OLD.user_id OR NEW.event_ordinal<>OLD.event_ordinal+1
       OR successor.before_version<>OLD.selection_version
       OR (successor.before_project_id,successor.before_item_id)
         IS DISTINCT FROM (OLD.selected_project_id,OLD.selected_item_id) THEN
      RAISE no_data_found;
    END IF;
  END IF;
  IF NEW.selection_version<>successor.selection_version
     OR (NEW.selected_project_id,NEW.selected_item_id)
       IS DISTINCT FROM (successor.selected_project_id,successor.selected_item_id) THEN
    RAISE no_data_found;
  END IF;
  RETURN NEW;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'invalid selection current transition'
    USING ERRCODE='23514',CONSTRAINT='selection_current_boundary';
END;
$$;
CREATE TRIGGER selection_current_transition BEFORE INSERT OR UPDATE ON kehila.current_work_selections
  FOR EACH ROW EXECUTE FUNCTION kehila.protect_selection_head();

CREATE FUNCTION kehila.check_selection_current_boundary() RETURNS trigger LANGUAGE plpgsql
SET search_path = pg_catalog, kehila, pg_temp AS $$
DECLARE head kehila.current_work_selections; event kehila.selection_events; latest kehila.u64;
BEGIN
  SELECT * INTO STRICT head FROM kehila.current_work_selections WHERE user_id=NEW.user_id;
  SELECT * INTO STRICT event FROM kehila.selection_events
    WHERE user_id=head.user_id AND event_ordinal=head.event_ordinal;
  SELECT event_ordinal INTO STRICT latest FROM kehila.selection_events
    WHERE user_id=head.user_id ORDER BY event_ordinal DESC LIMIT 1;
  IF latest<>head.event_ordinal OR event.selection_version<>head.selection_version
     OR (event.selected_project_id,event.selected_item_id)
       IS DISTINCT FROM (head.selected_project_id,head.selected_item_id) THEN
    RAISE no_data_found;
  END IF;
  RETURN NULL;
EXCEPTION WHEN NO_DATA_FOUND OR TOO_MANY_ROWS THEN
  RAISE EXCEPTION 'selection current head is not its latest accepted event'
    USING ERRCODE='23514',CONSTRAINT='selection_current_boundary';
END;
$$;
CREATE CONSTRAINT TRIGGER selection_current_boundary AFTER INSERT OR UPDATE ON kehila.current_work_selections
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION kehila.check_selection_current_boundary();
CREATE CONSTRAINT TRIGGER selection_event_current_boundary AFTER INSERT ON kehila.selection_events
  DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION kehila.check_selection_current_boundary();

REVOKE ALL ON FUNCTION
  kehila.selection_state_value(kehila.ident,kehila.u64,kehila.ident,kehila.ident),
  kehila.selection_u64_bytes(kehila.u64),
  kehila.selection_request_bytes(kehila.ident,kehila.operation_token,kehila.u64,kehila.ident,kehila.ident),
  kehila.protect_selection_history(),kehila.check_selection_event(),
  kehila.protect_selection_head(),kehila.check_selection_current_boundary() FROM PUBLIC;
