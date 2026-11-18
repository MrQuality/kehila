-- Fresh-v4 structural controls after the existing WorkItem storage fixture.
-- Owner DML deliberately tests constraints; it does not establish HTTP authority,
-- typed Rust acceptance, real response-loss recovery or browser qualification.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET lock_timeout='3s';
SET idle_in_transaction_session_timeout='15s';
SET ROLE kehila_owner;

DO $$ BEGIN
  IF (SELECT array_agg(version ORDER BY version) FROM kehila.schema_migrations)
       IS DISTINCT FROM ARRAY[1,2,3,4] THEN
    RAISE EXCEPTION 'exact fresh version-four selection fixture required';
  END IF;
END; $$;

CREATE FUNCTION pg_temp.require(ok boolean,label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'selection: %',label; END IF;
END;
$$;
CREATE FUNCTION pg_temp.reject(statement text,expected_state text,expected_constraint text)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual text; named text;
BEGIN
  BEGIN EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual=RETURNED_SQLSTATE,named=CONSTRAINT_NAME;
    IF actual<>expected_state OR named<>expected_constraint THEN
      RAISE EXCEPTION 'wrong selection rejection % / %, expected % / %',
        actual,named,expected_state,expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected selection rejection absent: % / %',expected_state,expected_constraint;
END;
$$;

SELECT pg_temp.require(encode(sha256(kehila.selection_request_bytes('owner',
  '0123456789abcdef0123456789abcdef',7,'project','item')),'hex')=
  'a72b12179c4d9db9c5901da14cec3c9ed424ef48ff520328d4fe329c936bb009',
  'SQL canonical frame agrees with the frozen Rust SHA-256 fixture');
SELECT pg_temp.require(encode(kehila.selection_u64_bytes(0::kehila.u64),'hex')='0000000000000000'
  AND encode(kehila.selection_u64_bytes(2147483648::kehila.u64),'hex')='0000000080000000'
  AND encode(kehila.selection_u64_bytes(4294967296::kehila.u64),'hex')='0000000100000000'
  AND encode(kehila.selection_u64_bytes(9223372036854775808::kehila.u64),'hex')='8000000000000000'
  AND encode(kehila.selection_u64_bytes(18446744073709551615::kehila.u64),'hex')='ffffffffffffffff',
  'unsigned request bytes preserve high-bit boundaries');

-- Independent construction of the fixed request frame. These controls use
-- expected version zero; full-u64/framing bounds belong to the pure codec cases.
CREATE FUNCTION pg_temp.clear_request(token text) RETURNS bytea LANGUAGE sql IMMUTABLE AS $$
  SELECT convert_to('kehila','UTF8')||decode('00','hex')||convert_to('selection','UTF8')
    ||decode('0000000001','hex')||int4send(5)||convert_to('owner','UTF8')
    ||int4send(octet_length(token))||convert_to(token,'UTF8')||int8send(0)||decode('00','hex');
$$;

CREATE FUNCTION pg_temp.clear_success(ordinal numeric,token text,advance_head boolean DEFAULT true) RETURNS bigint
LANGUAGE plpgsql AS $$
DECLARE operation bigint; result jsonb; history jsonb; request bytea;
BEGIN
  request:=pg_temp.clear_request(token);
  result:=jsonb_build_object('codec_version',1,'kind','selection_result','value',
    jsonb_build_object('user_id','owner','version','0','requested',NULL));
  history:=jsonb_build_object('codec_version',1,'kind','selection_event','value',
    jsonb_build_object('ordinal',ordinal::numeric(20,0)::text,'operation_id',token,
      'before',result->'value','result',result));
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_user_id,
    token,request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms)
    VALUES('owner','selection','user','owner',token,1,sha256(request),'selection',1,0,7776000000)
    RETURNING operation_row_id INTO operation;
  INSERT INTO kehila.operation_payloads(operation_row_id,request_bytes,result_codec_version,result)
    VALUES(operation,request,1,result);
  INSERT INTO kehila.selection_events(user_id,event_ordinal,previous_event_ordinal,
    actor_id,command_family,target_kind,token,operation_row_id,before_version,
    before_project_id,before_item_id,selection_version,selected_project_id,selected_item_id,
    history_codec_version,history)
    VALUES('owner',ordinal,CASE WHEN ordinal=1 THEN NULL ELSE ordinal-1 END,
      'owner','selection','user',token,operation,0,NULL,NULL,0,NULL,NULL,1,history);
  IF advance_head THEN
    INSERT INTO kehila.current_work_selections(user_id,selection_version,
      selected_project_id,selected_item_id,event_ordinal)
      VALUES('owner',0,NULL,NULL,ordinal)
      ON CONFLICT(user_id) DO UPDATE SET event_ordinal=EXCLUDED.event_ordinal;
  END IF;
  RETURN operation;
END;
$$;

CREATE FUNCTION pg_temp.orphan_success() RETURNS void LANGUAGE plpgsql AS $$
DECLARE operation bigint; request bytea:=pg_temp.clear_request(repeat('c',32));
BEGIN
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_user_id,
    token,request_codec_version,request_sha256,required_grant,grant_policy_version,
    replay_origin_ms,replay_deadline_ms)
    VALUES('owner','selection','user','owner',repeat('c',32),1,sha256(request),'selection',1,0,7776000000)
    RETURNING operation_row_id INTO operation;
  -- Keep payload shape/fingerprint valid so missing EVENT linkage is the only
  -- violated acceptance property; missing payload must not satisfy this case.
  INSERT INTO kehila.operation_payloads(operation_row_id,request_bytes,result_codec_version,result)
    VALUES(operation,request,1,jsonb_build_object('codec_version',1,'kind','selection_result','value',
      jsonb_build_object('user_id','owner','version','0','requested',NULL)));
END;
$$;

SELECT pg_temp.require(NOT EXISTS(SELECT FROM kehila.current_work_selections),
  'fresh owner has a virtual empty version-zero selection');

-- Two different successful clear operations each need permanent success and
-- event evidence, while the actual selection remains empty at version zero.
BEGIN;
SELECT pg_temp.clear_success(1,repeat('a',32));
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
BEGIN;
SELECT pg_temp.clear_success(2,repeat('b',32));
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT selection_version=0 AND event_ordinal=2
    AND selected_project_id IS NULL AND selected_item_id IS NULL
    FROM kehila.current_work_selections WHERE user_id='owner'),
  'successful no-op advances event evidence without changing selection version');
SELECT pg_temp.require((SELECT count(*)=2 FROM kehila.selection_events WHERE user_id='owner'),
  'both no-op events retained');
SELECT pg_temp.require((SELECT count(*)=2 FROM kehila.operations WHERE command_family='selection'),
  'both no-op operation cores retained');

SELECT pg_temp.reject($q$UPDATE kehila.selection_events SET token=repeat('c',32)
  WHERE user_id='owner' AND event_ordinal=1$q$,'23514','selection_history_immutable');
SELECT pg_temp.reject($q$DELETE FROM kehila.selection_events
  WHERE user_id='owner' AND event_ordinal=1$q$,'23514','selection_history_immutable');
-- Include both FK-linked tables so PostgreSQL's initial TRUNCATE FK admission
-- does not reject before the retained-history statement guard is reached.
SELECT pg_temp.reject($q$TRUNCATE kehila.current_work_selections,kehila.selection_events$q$,
  '23514','selection_history_immutable');
SELECT pg_temp.reject($q$UPDATE kehila.current_work_selections SET event_ordinal=1
  WHERE user_id='owner'; SET CONSTRAINTS ALL IMMEDIATE$q$,
  '23514','selection_current_boundary');
SELECT pg_temp.reject($q$UPDATE kehila.current_work_selections SET selection_version=1
  WHERE user_id='owner'; SET CONSTRAINTS ALL IMMEDIATE$q$,
  '23514','selection_current_boundary');
SELECT pg_temp.reject($q$DELETE FROM kehila.current_work_selections WHERE user_id='owner'$q$,
  '23514','selection_history_immutable');

-- Omitted-head and orphan-success controls must reach deferred closure rather
-- than pass because of unrelated malformed bytes or missing actor identity.
SELECT pg_temp.reject($q$SELECT pg_temp.orphan_success(); SET CONSTRAINTS ALL IMMEDIATE$q$,
  '23503','operation_success_mapping');
SELECT pg_temp.reject($q$SELECT pg_temp.clear_success(3,repeat('c',32),false);
  SET CONSTRAINTS ALL IMMEDIATE$q$,'23514','selection_current_boundary');

CREATE TEMP TABLE selection_unrelated_before AS SELECT jsonb_build_object(
  'items',(SELECT jsonb_agg(to_jsonb(i) ORDER BY i.item_id) FROM kehila.items i),
  'projects',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.project_id) FROM kehila.projects p),
  'revisions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY r.project_id,r.item_id,r.item_version) FROM kehila.item_revisions r),
  'heads',(SELECT jsonb_agg(to_jsonb(h) ORDER BY h.project_id) FROM kehila.project_operational_heads h)) AS snapshot;

CREATE FUNCTION pg_temp.accept_target(ordinal numeric,token text,project text,item text)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE before_state kehila.current_work_selections; after_version kehila.u64;
  operation bigint; request bytea; saved jsonb; history jsonb;
BEGIN
  SELECT * INTO STRICT before_state FROM kehila.current_work_selections WHERE user_id='owner';
  after_version:=before_state.selection_version+CASE WHEN
    (before_state.selected_project_id,before_state.selected_item_id) IS DISTINCT FROM (project,item) THEN 1 ELSE 0 END;
  request:=convert_to('kehila','UTF8')||decode('00','hex')||convert_to('selection','UTF8')||decode('0000000001','hex')
    ||int4send(5)||convert_to('owner','UTF8')||int4send(octet_length(token))||convert_to(token,'UTF8')
    ||int8send(before_state.selection_version::bigint)
    ||CASE WHEN project IS NULL THEN decode('00','hex') ELSE decode('01','hex')
      ||int4send(octet_length(project))||convert_to(project,'UTF8')||int4send(octet_length(item))||convert_to(item,'UTF8') END;
  saved:=jsonb_build_object('codec_version',1,'kind','selection_result','value',
    jsonb_build_object('user_id','owner','version',after_version::numeric(20,0)::text,'requested',
      CASE WHEN project IS NULL THEN 'null'::jsonb ELSE jsonb_build_object('project_id',project,'item_id',item) END));
  history:=jsonb_build_object('codec_version',1,'kind','selection_event','value',
    jsonb_build_object('ordinal',ordinal::numeric(20,0)::text,'operation_id',token,
      'before',jsonb_build_object('user_id','owner','version',before_state.selection_version::numeric(20,0)::text,
        'requested',CASE WHEN before_state.selected_project_id IS NULL THEN 'null'::jsonb ELSE jsonb_build_object(
          'project_id',before_state.selected_project_id,'item_id',before_state.selected_item_id) END),
      'result',saved));
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_user_id,token,
    request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES('owner','selection','user','owner',token,1,sha256(request),'selection',1,0,7776000000)
    RETURNING operation_row_id INTO operation;
  INSERT INTO kehila.operation_payloads(operation_row_id,request_bytes,result_codec_version,result)
    VALUES(operation,request,1,saved);
  INSERT INTO kehila.selection_events(user_id,event_ordinal,previous_event_ordinal,actor_id,command_family,
    target_kind,token,operation_row_id,before_version,before_project_id,before_item_id,selection_version,
    selected_project_id,selected_item_id,history_codec_version,history)
    VALUES('owner',ordinal,ordinal-1,'owner','selection','user',token,operation,before_state.selection_version,
      before_state.selected_project_id,before_state.selected_item_id,after_version,project,item,1,history);
  UPDATE kehila.current_work_selections SET selection_version=after_version,selected_project_id=project,
    selected_item_id=item,event_ordinal=ordinal WHERE user_id='owner';
END;
$$;

SELECT pg_temp.reject($q$SELECT pg_temp.accept_target(3,repeat('c',32),'p','absent-item')$q$,
  '23503','selection_event_target');
SELECT pg_temp.require((SELECT count(*)=2 FROM kehila.operations WHERE command_family='selection'),
  'target rejection leaves no partial success');
SELECT pg_temp.accept_target(3,repeat('d',32),'p','saved-item');
SELECT pg_temp.accept_target(4,repeat('e',32),'p','saved-item');
SELECT pg_temp.accept_target(5,repeat('f',32),'p','i');
SELECT pg_temp.accept_target(6,repeat('0',32),NULL,NULL);
SELECT pg_temp.require((SELECT selection_version=3 AND event_ordinal=6 AND selected_project_id IS NULL
  AND selected_item_id IS NULL FROM kehila.current_work_selections WHERE user_id='owner'),
  'set A, no-op A, set B and clear preserve independent version and event order');
SELECT pg_temp.require((SELECT array_agg(selection_version::numeric ORDER BY event_ordinal)=ARRAY[0,0,1,1,2,3]::numeric[]
  FROM kehila.selection_events WHERE user_id='owner'),'exact selection transition versions');
SELECT pg_temp.require((SELECT snapshot FROM selection_unrelated_before)=jsonb_build_object(
  'items',(SELECT jsonb_agg(to_jsonb(i) ORDER BY i.item_id) FROM kehila.items i),
  'projects',(SELECT jsonb_agg(to_jsonb(p) ORDER BY p.project_id) FROM kehila.projects p),
  'revisions',(SELECT jsonb_agg(to_jsonb(r) ORDER BY r.project_id,r.item_id,r.item_version) FROM kehila.item_revisions r),
  'heads',(SELECT jsonb_agg(to_jsonb(h) ORDER BY h.project_id) FROM kehila.project_operational_heads h)),
  'selection changes no item, lifecycle, project or operational history');

-- This role must not acquire owner-defined mutation/DDL privileges merely
-- because the fresh product includes selection.
-- Owner default ACLs also protect temporary fixture functions. Grant only this
-- invoker assertion helper so the role checks reach their intended predicates;
-- do not change any product function or table grant to satisfy the fixture.
GRANT EXECUTE ON FUNCTION pg_temp.require(boolean,text) TO kehila_auth_runtime;
SET ROLE kehila_auth_runtime;
SELECT pg_temp.require(NOT has_table_privilege(current_user,'kehila.selection_events',
  'INSERT,UPDATE,DELETE,TRUNCATE'), 'runtime cannot directly mutate selection events');
SELECT pg_temp.require(NOT has_table_privilege(current_user,'kehila.current_work_selections',
  'INSERT,UPDATE,DELETE,TRUNCATE'), 'runtime cannot directly mutate selection head');
SELECT pg_temp.require(NOT has_schema_privilege(current_user,'kehila','CREATE'),
  'runtime cannot change selection schema');
RESET ROLE;
