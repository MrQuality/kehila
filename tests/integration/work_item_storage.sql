-- Owner-only structural controls in a fresh synthetic product database after
-- enrollment and installation of the reviewed WorkItem storage modules.
-- This is not typed-codec, serving-role, authorization or HTTP qualification.
-- The item core uses a constructed past origin to exercise retirement shape;
-- this is not evidence that real time elapsed or that runtime accepted expiry.
\set ON_ERROR_STOP on
SET statement_timeout = '15s';
SET lock_timeout = '3s';
SET idle_in_transaction_session_timeout = '15s';
SET ROLE kehila_owner;

CREATE FUNCTION pg_temp.require(ok boolean, label text) RETURNS void
LANGUAGE plpgsql AS $$
BEGIN
  IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'WorkItem structure: %', label; END IF;
END;
$$;
CREATE FUNCTION pg_temp.reject(statement text, expected_state text, expected_constraint text)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual_state text; actual_constraint text;
BEGIN
  BEGIN
    EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual_state = RETURNED_SQLSTATE,
                           actual_constraint = CONSTRAINT_NAME;
    IF actual_state <> expected_state OR actual_constraint <> expected_constraint THEN
      RAISE EXCEPTION 'wrong WorkItem rejection: % / %, expected % / %',
        actual_state, actual_constraint, expected_state, expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected WorkItem rejection absent: % / %', expected_state, expected_constraint;
END;
$$;

SELECT pg_temp.require((SELECT count(*) = 0 FROM kehila.projects), 'fresh project fixture');
SELECT pg_temp.require((SELECT owner_actor_id = 'owner' AND installation_id = 'installation'
  FROM kehila_auth.installation WHERE singleton), 'synthetic enrolled owner');

BEGIN;
DO $$
DECLARE registered_request bigint; operation_id bigint; at_ms kehila.u64;
BEGIN
  at_ms := kehila_auth.trusted_millis();
  INSERT INTO kehila_auth.pending_projects
    (installation_id, actor_id, intent_handle, token, name, prefix, estimate_unit,
     registered_ms, expires_ms)
    VALUES ('installation', 'owner', repeat('a',32), repeat('a',32), 'project', 'PR', 'hours',
      at_ms, at_ms + 86400000) RETURNING pending_projects.request_id INTO registered_request;
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','project_create','project','p',repeat('a',32),1,
      sha256('\x01'::bytea),'project_create',1,at_ms,at_ms+7776000000)
    RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES (operation_id,'\x01',1,'{}');
  INSERT INTO kehila.projects
    (project_id,name,current_prefix,estimate_unit,configuration_revision,next_sequence,
     created_by,creation_operation_row_id,seed_profile)
    VALUES ('p','project','PR','hours',1,1,'owner',operation_id,'m1_v1');
  INSERT INTO kehila.configuration_revisions VALUES
    ('p',1,1,'{}',
     '{"codec_version":1,"kind":"project_snapshot","value":{"id":"p","name":"project","prefix":"PR","configuration_revision":"1","next_sequence":"1","archived":false,"estimate_unit":"hours","ever_estimated":false}}',
     'owner',operation_id,'project_create','project');
  INSERT INTO kehila.project_grants VALUES ('p','owner','initial_owner',true,1);
  INSERT INTO kehila.project_grant_events
    (project_id,actor_id,version,access_profile,active,granted_by,granting_operation_row_id,command_family)
    VALUES ('p','owner',1,'initial_owner',true,'owner',operation_id,'project_create');
  INSERT INTO kehila.statuses VALUES ('p','new','New','new',false,0);
  INSERT INTO kehila.workflows VALUES ('p','wf','new','new',false,0);
  INSERT INTO kehila.workflow_statuses VALUES ('p','wf','new',0);
  INSERT INTO kehila.work_item_types VALUES ('p','task','wf','title',false,0);
  INSERT INTO kehila.type_workflows VALUES ('p','task','wf',0);
  INSERT INTO kehila.fields VALUES ('p','title','task','Title','text','application','optional',false,0);
  UPDATE kehila_auth.pending_projects SET state='completed',target_project_id='p',operation_row_id=operation_id
    WHERE pending_projects.request_id = registered_request;
  INSERT INTO kehila_auth.creation_intents VALUES
    (registered_request,'owner','project_create',repeat('a',32),'project','p',operation_id);
END;
$$;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT h.state_version=0 AND h.operation_row_id=p.creation_operation_row_id
  FROM kehila.projects p JOIN kehila.project_operational_heads h USING (project_id)
  WHERE p.project_id='p'), 'ProjectCreate seeds an attributed zero head');

BEGIN;
DO $$
DECLARE
  registered_request bigint; operation_id bigint; previous_operation bigint; at_ms kehila.u64;
  request bytea := convert_to('{"codec_version":1,"kind":"item_create_request","value":{"estimate":"0","expected_configuration_revision":"1","item_id":"i","operation_id":"op","project_id":"p","type_id":"task","values":[],"workflow_id":null}}','UTF8');
  saved jsonb := '{"codec_version":1,"kind":"item_create_result","value":{"project":{"id":"p","name":"project","prefix":"PR","configuration_revision":"1","next_sequence":"2","archived":false,"estimate_unit":"hours","ever_estimated":true},"item":{"id":"i","project_id":"p","type_id":"task","workflow_id":"wf","status_id":"new","version":"1","archived":false},"data":{"estimate":"0","values":[]},"phase":"new","readable_id":{"project_id":"p","item_id":"i","prefix":"PR","sequence":"1"}}}';
BEGIN
  at_ms := kehila_auth.trusted_millis();
  SELECT h.operation_row_id INTO STRICT previous_operation
    FROM kehila.project_operational_heads h WHERE h.project_id='p';
  INSERT INTO kehila_auth.pending_items
    (installation_id,actor_id,project_id,intent_handle,token,target_item_id,
     request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
    VALUES ('installation','owner','p',repeat('b',32),'op','i',1,sha256(request),request,
      at_ms,at_ms+86400000) RETURNING pending_items.request_id INTO registered_request;
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,target_item_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','item_create','work_item','p','i','op',1,sha256(request),'item_create',1,0,7776000000)
    RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES (operation_id,request,1,saved);
  INSERT INTO kehila.items VALUES ('i','p','PR',1,registered_request,operation_id,1);
  INSERT INTO kehila.item_revisions
    (project_id,item_id,item_version,configuration_revision,actor_id,command_family,token,
     operation_row_id,type_id,workflow_id,status_id,phase,archived,estimate_thousandths,
     estimate_unit,history_codec_version,history,project_state_version,previous_project_operation,
     resulting_next_sequence,resulting_ever_estimated)
    VALUES ('p','i',1,1,'owner','item_create','op',operation_id,'task','wf','new','new',false,0,
      'hours',1,jsonb_build_object('codec_version',1,'kind','item_revision','value',
        jsonb_build_object('actor_id','owner','configuration_revision','1','project_state_version','1',
          'previous_project_operation',previous_operation::text,'result',saved)),1,previous_operation,2,true);
  UPDATE kehila.projects SET next_sequence=2,ever_estimated=true WHERE project_id='p';
  UPDATE kehila.project_operational_heads SET state_version=1,operation_row_id=operation_id WHERE project_id='p';
  UPDATE kehila_auth.pending_items SET state='completed',operation_row_id=operation_id
    WHERE pending_items.request_id=registered_request;
END;
$$;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT count(*)=1 FROM kehila.items), 'committed structural item');
SELECT pg_temp.require((SELECT estimate_thousandths=0 AND resulting_ever_estimated
  FROM kehila.item_revisions), 'zero is a present estimate witness');

SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,target_item_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','item_create','work_item','p','orphan','orphan',1,sha256('\x01'::bytea),
      'item_create',1,0,7776000000);
  INSERT INTO kehila.operation_payloads
    SELECT operation_row_id,'\x01',1,'{}' FROM kehila.operations WHERE token='orphan';
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23503', 'operation_success_mapping');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,token,request_codec_version,
     request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','project_create','project','orphan-project','orphan-project',1,
      sha256('\x01'::bytea),'project_create',1,0,7776000000);
  INSERT INTO kehila.operation_payloads
    SELECT operation_row_id,'\x01',1,'{}' FROM kehila.operations WHERE token='orphan-project';
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23503', 'operation_success_mapping');
SELECT pg_temp.reject($q$
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,target_item_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','item_edit','work_item','p','i','orphan-edit',1,sha256('\x01'::bytea),
      'item_edit',1,0,7776000000);
  INSERT INTO kehila.operation_payloads
    SELECT operation_row_id,'\x01',1,'{}' FROM kehila.operations WHERE token='orphan-edit';
  SET CONSTRAINTS ALL IMMEDIATE
$q$, '23503', 'operation_success_mapping');
SELECT pg_temp.reject($q$
  DELETE FROM kehila.operation_payloads WHERE operation_row_id IN
    (SELECT operation_row_id FROM kehila.operations WHERE target_item_id='i')
$q$, '23514', 'operation_payload_retirement_required');
SELECT pg_temp.reject($q$
  UPDATE kehila.project_operational_heads SET state_version=0 WHERE project_id='p'
$q$, '23514', 'project_operational_transition');
SELECT pg_temp.reject($q$
  UPDATE kehila.project_operational_heads SET state_version=3 WHERE project_id='p'
$q$, '23514', 'project_operational_transition');
SELECT pg_temp.reject($q$
  UPDATE kehila.projects SET ever_estimated=false WHERE project_id='p'; SET CONSTRAINTS ALL IMMEDIATE
$q$, '23514', 'project_current_boundary');
SELECT pg_temp.reject($q$
  UPDATE kehila.items SET current_version=3 WHERE item_id='i'
$q$, '23514', 'item_identity_transition');
SELECT pg_temp.reject($q$
  UPDATE kehila.items SET issued_prefix='OTHER',current_version=2 WHERE item_id='i'
$q$, '23514', 'item_identity_transition');
SELECT pg_temp.reject($q$DELETE FROM kehila.item_revisions$q$, '23514', 'item_history_immutable');
SELECT pg_temp.reject($q$DELETE FROM kehila_auth.pending_items$q$, '23514', 'pending_item_transition');
SELECT pg_temp.reject($q$
  UPDATE kehila_auth.pending_items SET request_bytes=NULL WHERE target_item_id='i'
$q$, '23514', 'pending_item_transition');

BEGIN;
DO $$
DECLARE
  operation_id bigint; previous_operation bigint;
  request bytea := convert_to('{"codec_version":1,"kind":"item_edit_request","value":{"estimate":{"action":"clear"},"expected_configuration_revision":"1","expected_item_version":"1","fields":[],"item_id":"i","operation_id":"edit-op","project_id":"p"}}','UTF8');
  saved jsonb := '{"codec_version":1,"kind":"item_edit_result","value":{"project":{"id":"p","name":"project","prefix":"PR","configuration_revision":"1","next_sequence":"2","archived":false,"estimate_unit":"hours","ever_estimated":true},"item":{"id":"i","project_id":"p","type_id":"task","workflow_id":"wf","status_id":"new","version":"2","archived":false},"data":{"estimate":null,"values":[]},"phase":"new"}}';
BEGIN
  SELECT h.operation_row_id INTO STRICT previous_operation
    FROM kehila.project_operational_heads h WHERE h.project_id='p';
  INSERT INTO kehila.operations
    (actor_id,command_family,target_kind,target_project_id,target_item_id,token,
     request_codec_version,request_sha256,required_grant,grant_policy_version,
     replay_origin_ms,replay_deadline_ms)
    VALUES ('owner','item_edit','work_item','p','i','edit-op',1,sha256(request),'item_edit',1,0,7776000000)
    RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES (operation_id,request,1,saved);
  INSERT INTO kehila.item_revisions
    (project_id,item_id,item_version,previous_item_version,configuration_revision,actor_id,
     command_family,token,operation_row_id,type_id,workflow_id,status_id,phase,archived,
     estimate_thousandths,estimate_unit,history_codec_version,history,project_state_version,
     previous_project_operation,resulting_next_sequence,resulting_ever_estimated)
    VALUES ('p','i',2,1,1,'owner','item_edit','edit-op',operation_id,'task','wf','new','new',false,
      NULL,'hours',1,jsonb_build_object('codec_version',1,'kind','item_revision','value',
        jsonb_build_object('actor_id','owner','configuration_revision','1','project_state_version','2',
          'previous_project_operation',previous_operation::text,'result',saved)),2,previous_operation,2,true);
  UPDATE kehila.items SET current_version=2 WHERE item_id='i';
  UPDATE kehila.project_operational_heads SET state_version=2,operation_row_id=operation_id WHERE project_id='p';
END;
$$;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT i.current_version=2 AND h.state_version=2
    AND p.next_sequence=2 AND p.ever_estimated
  FROM kehila.items i JOIN kehila.projects p USING (project_id)
    JOIN kehila.project_operational_heads h USING (project_id) WHERE i.item_id='i'),
  'edit advances both heads without consuming a sequence or clearing estimate history');
SELECT pg_temp.require((SELECT estimate_thousandths IS NULL FROM kehila.item_revisions
  WHERE item_id='i' AND item_version=2), 'cleared estimate remains absent');

-- Retire the constructed historical core, retaining pending content until a
-- separate transaction. These owner writes do not qualify a serving capability.
BEGIN;
UPDATE kehila.operations SET payload_retired=true WHERE target_item_id='i';
DELETE FROM kehila.operation_payloads WHERE operation_row_id IN
  (SELECT operation_row_id FROM kehila.operations WHERE target_item_id='i');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT request_bytes IS NOT NULL FROM kehila_auth.pending_items),
  'retired replay permits temporarily retained pending content');
BEGIN;
UPDATE kehila_auth.pending_items SET request_bytes=NULL WHERE target_item_id='i';
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require((SELECT state='completed' AND operation_row_id IS NOT NULL
  AND request_bytes IS NULL AND target_item_id='i' FROM kehila_auth.pending_items),
  'content pruning preserves successful identity');
SELECT pg_temp.require((SELECT count(*)=2 AND bool_and(history IS NOT NULL)
  FROM kehila.item_revisions), 'product history survives replay retirement');
RESET ROLE;
SELECT 'PASS: WorkItem owner-only storage structure and retained identity' AS result;
