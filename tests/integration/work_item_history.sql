-- Owner-only history controls after work_item_storage.sql and its observer.
-- This does not qualify a serving command or current configuration snapshot.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET ROLE kehila_owner;
CREATE FUNCTION pg_temp.require(ok boolean, label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'history control: %',label; END IF;
END;
$$;
CREATE FUNCTION pg_temp.reject(statement text, expected_constraint text) RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual_state text; actual_constraint text;
BEGIN
  BEGIN
    EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual_state=RETURNED_SQLSTATE,actual_constraint=CONSTRAINT_NAME;
    IF actual_state<>'23514' OR actual_constraint<>expected_constraint THEN
      RAISE EXCEPTION 'wrong history rejection: % / %, expected 23514 / %',
        actual_state,actual_constraint,expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected history rejection absent: %',expected_constraint;
END;
$$;

-- Otherwise-valid alternate references prevent ordinary-edit substitution
-- controls from being satisfied by an unrelated foreign-key rejection.
BEGIN;
INSERT INTO kehila.statuses VALUES ('p','alternate','Alternate','new',false,1);
INSERT INTO kehila.workflow_statuses VALUES ('p','wf','alternate',1);
INSERT INTO kehila.workflows VALUES ('p','other-wf','new','new',false,1);
INSERT INTO kehila.workflow_statuses VALUES ('p','other-wf','new',0);
INSERT INTO kehila.type_workflows VALUES ('p','task','other-wf',1);
INSERT INTO kehila.work_item_types VALUES ('p','milestone','wf',NULL,false,1);
INSERT INTO kehila.type_workflows VALUES ('p','milestone','wf',0);
INSERT INTO kehila.fields VALUES ('p','other-title','milestone','Other','text','application','optional',false,1);
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;

CREATE FUNCTION pg_temp.append_edit(
  data_values jsonb DEFAULT '[]', command_fields text DEFAULT '[]', record_use boolean DEFAULT true,
  patch_path text[] DEFAULT NULL, patch_value jsonb DEFAULT NULL,
  item_type text DEFAULT 'task', item_workflow text DEFAULT 'wf', item_status text DEFAULT 'new',
  config_scale numeric DEFAULT 1, replay_version numeric DEFAULT 1
) RETURNS void LANGUAGE plpgsql AS $$
DECLARE
  previous_operation bigint; operation_id bigint; next_item_version kehila.u64; next_state_version kehila.u64;
  operation_token text; request bytea; saved_result jsonb; new_history jsonb; entry jsonb;
BEGIN
  SELECT i.current_version+1,h.state_version+1,h.operation_row_id
    INTO STRICT next_item_version,next_state_version,previous_operation
    FROM kehila.items i JOIN kehila.project_operational_heads h USING(project_id) WHERE i.item_id='i';
  operation_token := 'history-' || next_state_version::text;
  request := convert_to(format('{"codec_version":1,"kind":"item_edit_request","value":{"estimate":{"action":"keep"},"expected_configuration_revision":"1","expected_item_version":%s,"fields":%s,"item_id":"i","operation_id":%s,"project_id":"p"}}',
    to_jsonb((next_item_version-1)::text)::text,command_fields,to_jsonb(operation_token)::text),'UTF8');
  saved_result := jsonb_build_object('codec_version',1,'kind','item_edit_result','value',jsonb_build_object(
    'project','{"id":"p","name":"project","prefix":"PR","configuration_revision":"1","next_sequence":"2","archived":false,"estimate_unit":"hours","ever_estimated":true}'::jsonb,
    'item',jsonb_build_object('id','i','project_id','p','type_id',item_type,'workflow_id',item_workflow,
      'status_id',item_status,'version',next_item_version::text,'archived',false),
    'data',jsonb_build_object('estimate',NULL,'values',data_values),'phase','new'));
  new_history := jsonb_build_object('codec_version',1,'kind','item_revision','value',jsonb_build_object(
    'actor_id','owner','configuration_revision','1','project_state_version',next_state_version::text,
    'previous_project_operation',previous_operation::text,'result',saved_result));
  IF patch_path IS NOT NULL THEN
    IF patch_value IS NULL THEN new_history := new_history #- patch_path;
    ELSE new_history := jsonb_set(new_history,patch_path,patch_value); END IF;
  END IF;
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,target_item_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','item_edit','work_item','p','i',operation_token,1,sha256(request),'item_edit',1,0,7776000000)
    RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES (operation_id,request,1,
    jsonb_set(new_history #>'{value,result}','{codec_version}',to_jsonb(replay_version)));
  INSERT INTO kehila.item_revisions
    (project_id,item_id,item_version,previous_item_version,configuration_revision,actor_id,command_family,
     token,operation_row_id,type_id,workflow_id,status_id,phase,archived,estimate_thousandths,estimate_unit,
     history_codec_version,history,project_state_version,previous_project_operation,resulting_next_sequence,
     resulting_ever_estimated)
    VALUES ('p','i',next_item_version,next_item_version-1,config_scale,'owner','item_edit',operation_token,operation_id,item_type,item_workflow,
      item_status,'new',false,NULL,'hours',1,new_history,next_state_version,previous_operation,2,true);
  IF record_use THEN
    FOR entry IN SELECT value FROM jsonb_array_elements(data_values) LOOP
      INSERT INTO kehila.item_field_uses VALUES ('p',item_type,entry->>'id','i',next_item_version,operation_id)
        ON CONFLICT DO NOTHING;
    END LOOP;
  END IF;
  UPDATE kehila.items SET current_version=next_item_version WHERE item_id='i';
  UPDATE kehila.project_operational_heads SET state_version=next_state_version,
    operation_row_id=operation_id WHERE project_id='p';
END;
$$;

-- With the history guard absent these complete writes are accepted by the
-- other structural guards, so the expected rejection cannot be a missing FK.
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{value,actor_id}',patch_value=>'"other"');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{value,configuration_revision}',patch_value=>'1');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{codec_version}',patch_value=>'1.0');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{value,result,codec_version}',patch_value=>'1.0');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(replay_version=>1.0);
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_replay_history_agreement');
-- Deliberately corrupt a retained snapshot only inside the rejection's rollback
-- scope. This probes classification/readability, not protection against DDL.
SELECT pg_temp.reject($q$
  ALTER TABLE kehila.configuration_revisions DISABLE TRIGGER configuration_history_immutable;
  UPDATE kehila.configuration_revisions SET project_snapshot=jsonb_set(project_snapshot,'{codec_version}','1.0');
  SELECT pg_temp.append_edit(); SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.require((SELECT tgenabled='O' FROM pg_trigger
  WHERE tgrelid='kehila.configuration_revisions'::regclass AND tgname='configuration_history_immutable'),
  'rejected corruption probe restored snapshot protection');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{value,result,kind}',patch_value=>'"item_create_result"');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{value,result,value,data,estimate}');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(patch_path=>'{value,result,value,project,name}',patch_value=>'"changed"');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_agreement');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(item_type=>'milestone');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_edit_lifecycle');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(item_workflow=>'other-wf');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_edit_lifecycle');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(item_status=>'alternate');
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_edit_lifecycle');
SELECT pg_temp.reject($q$INSERT INTO kehila.item_field_uses
  SELECT 'p','task','title','i',2,operation_row_id FROM kehila.item_revisions WHERE item_version=2;
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_field_use_reference');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(
  data_values=>'[{"id":"title","value":{"kind":"text","value":""}}]',record_use=>false);
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_field_use_missing');
SELECT pg_temp.reject($q$SELECT pg_temp.append_edit(
  data_values=>'[{"id":"other-title","value":{"kind":"text","value":"value"}}]',record_use=>false);
  SET CONSTRAINTS ALL IMMEDIATE$q$,'item_history_field_reference');

BEGIN;
SELECT pg_temp.append_edit(data_values=>'[{"id":"title","value":{"kind":"text","value":""}}]',
  command_fields=>'[{"edit":{"action":"set","value":{"kind":"text","value":""}},"id":"title"}]');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
BEGIN;
SELECT pg_temp.append_edit(command_fields=>'[{"edit":{"action":"clear"},"id":"title"}]');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT count(*)=1 AND bool_and(first_item_version=3) FROM kehila.item_field_uses),
  'empty text is valued and its first use survives clearing');

-- Stored snapshots can retain more values than a single sparse command can
-- supply. This transaction tests representation bounds, not command acceptance.
BEGIN;
INSERT INTO kehila.fields
  SELECT 'p','detail-'||n,'task','Detail '||n,'text','application','optional',false,n+1
    FROM generate_series(1,129) n;
SELECT pg_temp.append_edit(data_values=>(SELECT jsonb_agg(jsonb_build_object('id','detail-'||n,
    'value',jsonb_build_object('kind','text','value','value')) ORDER BY n) FROM generate_series(1,129)n));
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;

-- An integral PostgreSQL numeric can preserve scale while the frozen JSON
-- remains the canonical logical-u64 string. This is not a new JSON spelling.
BEGIN;
SELECT pg_temp.append_edit(config_scale=>1.0);
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;

BEGIN;
UPDATE kehila.operations SET payload_retired=true WHERE command_family='item_edit' AND NOT payload_retired;
DELETE FROM kehila.operation_payloads WHERE operation_row_id IN
  (SELECT operation_row_id FROM kehila.operations WHERE payload_retired);
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
RESET ROLE;
SELECT 'PASS: history agreement, ordinary-edit lifecycle and retained valued-field use' AS result;
