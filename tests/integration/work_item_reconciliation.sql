-- Owner-authored historical resolution controls, not application qualification.
-- The third accepted fixture uses a constructed past replay origin.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET ROLE kehila_owner;
CREATE FUNCTION pg_temp.require(ok boolean,label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'reconciliation: %',label; END IF; END;
$$;
CREATE FUNCTION pg_temp.reject(statement text,expected_state text) RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual text;
BEGIN
  BEGIN EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual=RETURNED_SQLSTATE;
    IF actual<>expected_state THEN RAISE EXCEPTION 'wrong reconciliation rejection %, expected %',actual,expected_state; END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected reconciliation rejection absent: %',expected_state;
END;
$$;
CREATE FUNCTION pg_temp.authority() RETURNS kehila_auth.authority LANGUAGE sql STABLE AS $$
  SELECT ROW(i.installation_id,i.owner_actor_id,i.runtime_generation,c.auth_generation,c.verifier_revision)::kehila_auth.authority
    FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id);
$$;
CREATE FUNCTION pg_temp.bad_authority(field text,value jsonb) RETURNS kehila_auth.authority LANGUAGE sql STABLE AS $$
  SELECT jsonb_populate_record(NULL::kehila_auth.authority,to_jsonb(pg_temp.authority())||jsonb_build_object(field,value));
$$;
CREATE TEMP TABLE requests(version integer PRIMARY KEY,token text,bytes bytea,operation_id bigint);
CREATE FUNCTION pg_temp.accept_fixture(n integer) RETURNS void LANGUAGE plpgsql AS $$
DECLARE op_token text:=repeat((10-n)::text,32); family text; request bytea; saved jsonb;
  pending_id bigint; operation_id bigint; previous_operation bigint; next_state kehila.u64;
  origin kehila.u64; estimate integer; data_estimate jsonb;
BEGIN
  family:=CASE WHEN n=1 THEN 'item_create' ELSE 'item_edit' END;
  origin:=CASE WHEN n=3 THEN 0 ELSE kehila_auth.trusted_millis() END;
  estimate:=CASE WHEN n=2 THEN 0 ELSE NULL END;
  data_estimate:=coalesce(to_jsonb(estimate::text),'null'::jsonb);
  SELECT h.operation_row_id,h.state_version+1 INTO STRICT previous_operation,next_state
    FROM kehila.project_operational_heads h WHERE h.project_id='p';
  IF n=1 THEN
    request:=convert_to(format('{"codec_version":1,"kind":"item_create_request","value":{"estimate":null,"expected_configuration_revision":"1","item_id":"live-item","operation_id":"%s","project_id":"p","type_id":"task","values":[],"workflow_id":null}}',op_token),'UTF8');
    INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,token,
      target_item_id,request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
      VALUES('installation','owner','p',op_token,op_token,'live-item',1,sha256(request),request,origin,origin+86400000)
      RETURNING request_id INTO pending_id;
  ELSE
    request:=convert_to(format('{"codec_version":1,"kind":"item_edit_request","value":{"estimate":%s,"expected_configuration_revision":"1","expected_item_version":"%s","fields":[],"item_id":"live-item","operation_id":"%s","project_id":"p"}}',
      CASE WHEN n=2 THEN '{"action":"set","value":"0"}' ELSE '{"action":"clear"}' END,n-1,op_token),'UTF8');
  END IF;
  saved:=jsonb_build_object('codec_version',1,'kind',family||'_result','value',jsonb_build_object(
    'project','{"id":"p","name":"project","prefix":"PR","configuration_revision":"1","next_sequence":"3","archived":false,"estimate_unit":"hours","ever_estimated":true}'::jsonb,
    'item',jsonb_build_object('id','live-item','project_id','p','type_id','task','workflow_id','wf',
      'status_id','new','version',n::text,'archived',false),
    'data',jsonb_build_object('estimate',data_estimate,'values','[]'::jsonb),'phase','new'));
  IF n=1 THEN
    saved:=jsonb_set(saved,'{value}',(saved->'value')||jsonb_build_object('readable_id',
      jsonb_build_object('project_id','p','item_id','live-item','prefix','PR','sequence','2')));
  END IF;
  INSERT INTO kehila.operations(actor_id,command_family,target_kind,target_project_id,target_item_id,
    token,request_codec_version,request_sha256,required_grant,grant_policy_version,replay_origin_ms,replay_deadline_ms)
    VALUES('owner',family,'work_item','p','live-item',op_token,1,sha256(request),family,1,origin,origin+7776000000)
    RETURNING operation_row_id INTO operation_id;
  INSERT INTO kehila.operation_payloads VALUES(operation_id,request,1,saved);
  IF n=1 THEN
    INSERT INTO kehila.items VALUES('live-item','p','PR',2,pending_id,operation_id,1);
  END IF;
  INSERT INTO kehila.item_revisions(project_id,item_id,item_version,previous_item_version,
    configuration_revision,actor_id,command_family,token,operation_row_id,type_id,workflow_id,status_id,
    phase,archived,estimate_thousandths,estimate_unit,history_codec_version,history,project_state_version,
    previous_project_operation,resulting_next_sequence,resulting_ever_estimated)
    VALUES('p','live-item',n,CASE WHEN n=1 THEN NULL ELSE n-1 END,1,'owner',family,op_token,operation_id,
      'task','wf','new','new',false,estimate,'hours',1,jsonb_build_object('codec_version',1,'kind','item_revision',
        'value',jsonb_build_object('actor_id','owner','configuration_revision','1',
          'project_state_version',next_state::text,'previous_project_operation',previous_operation::text,'result',saved)),
      next_state,previous_operation,3,true);
  IF n=1 THEN
    UPDATE kehila.projects SET next_sequence=3 WHERE project_id='p';
    UPDATE kehila_auth.pending_items SET state='completed',operation_row_id=operation_id
      WHERE request_id=pending_id;
  ELSE
    UPDATE kehila.items SET current_version=n WHERE item_id='live-item';
  END IF;
  UPDATE kehila.project_operational_heads SET state_version=next_state,operation_row_id=operation_id WHERE project_id='p';
  INSERT INTO requests VALUES(n,op_token,request,operation_id);
END;
$$;
BEGIN;
DO $$ BEGIN
  FOR n IN 1..3 LOOP PERFORM pg_temp.accept_fixture(n); END LOOP;
END; $$;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
CREATE TEMP TABLE before_resolution AS SELECT request_id,xmin::text AS row_version FROM kehila_auth.pending_items;

BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.require((SELECT outcome='replay' AND target_item_id='live-item' AND operation_row_id IS NOT NULL
  FROM kehila_auth.reconcile_item_create(pg_temp.authority(),'p',repeat('9',32))),
  'creation resolves historical version one after subsequent edits');
SELECT pg_temp.require((SELECT r.outcome='replay' FROM requests q,
  LATERAL kehila_auth.reconcile_item_edit(pg_temp.authority(),'p','live-item',q.token::kehila.operation_token,1,q.bytes) r
  WHERE q.version=2),'live direct edit resolves before current version eligibility');
SELECT pg_temp.require((SELECT r.outcome='conflict' FROM requests q,
  LATERAL kehila_auth.reconcile_item_edit(pg_temp.authority(),'p','live-item',q.token::kehila.operation_token,1,q.bytes||convert_to(' ','UTF8')) r
  WHERE q.version=2),'changed bytes under existing identity conflict');
SELECT pg_temp.require((SELECT r.outcome='expired' FROM requests q,
  LATERAL kehila_auth.reconcile_item_edit(pg_temp.authority(),'p','live-item',q.token::kehila.operation_token,1,q.bytes) r
  WHERE q.version=3),'logical expiry with retained payload never becomes unseen');
SELECT pg_temp.require((SELECT outcome='expired' FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p',repeat('b',32))),'retired creation preserves terminal identity');
SELECT pg_temp.require((SELECT outcome='conflict' FROM kehila_auth.reconcile_item_edit(
  pg_temp.authority(),'p','i','edit-op',1,convert_to('{}','UTF8'))),
  'permanent changed identity conflicts after retirement');
SELECT pg_temp.require((SELECT outcome='prepared' FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p','00000000000000000000000000000002')),'prepared intent is never executed');
SELECT pg_temp.require((SELECT outcome='cancelled' FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p','00000000000000000000000000000001')),'cancelled intent stays terminal');
SELECT pg_temp.require((SELECT outcome='expired' FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p',repeat('e',32))),'elapsed prepared intent cannot execute');
SELECT pg_temp.require((SELECT outcome='missing' AND target_item_id IS NULL FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p',repeat('a',32))),'missing locator never allocates identity');
SELECT pg_temp.require((SELECT outcome='unseen' FROM kehila_auth.reconcile_item_edit(
  pg_temp.authority(),'p','live-item','unseen',1,convert_to('{}','UTF8'))),'unseen edit does not execute');
SELECT pg_temp.require((SELECT outcome='missing' FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p2',repeat('9',32))),'creation lookup remains project scoped');
SELECT pg_temp.require((SELECT r.outcome='unseen' FROM requests q,
  LATERAL kehila_auth.reconcile_item_edit(pg_temp.authority(),'p2','live-item',q.token::kehila.operation_token,1,q.bytes) r
  WHERE q.version=2),'edit lookup remains project scoped');
SELECT pg_temp.require((SELECT outcome='expired' FROM kehila_auth.reconcile_item_edit(
  pg_temp.authority(),'p','i','edit-op',1,convert_to('{"codec_version":1,"kind":"item_edit_request","value":{"estimate":{"action":"clear"},"expected_configuration_revision":"1","expected_item_version":"1","fields":[],"item_id":"i","operation_id":"edit-op","project_id":"p"}}','UTF8'))),
  'equal retired edit remains expired');
DO $$
DECLARE field text; stale_value jsonb;
BEGIN
  FOREACH field IN ARRAY ARRAY['installation_id','owner_actor_id','runtime_generation','auth_generation','verifier_revision'] LOOP
    PERFORM pg_temp.reject(format('SELECT * FROM kehila_auth.reconcile_item_create(pg_temp.bad_authority(%L,''null''),''p'',repeat(''9'',32))',field),'55000');
    PERFORM pg_temp.reject(format('SELECT * FROM kehila_auth.reconcile_item_edit(pg_temp.bad_authority(%L,''null''),''p'',''live-item'',repeat(''8'',32),1,convert_to(''{}'',''UTF8''))',field),'55000');
  END LOOP;
  FOREACH field IN ARRAY ARRAY['runtime_generation','auth_generation','verifier_revision'] LOOP
    stale_value:=to_jsonb((to_jsonb(pg_temp.authority())->>field)::numeric+1);
    PERFORM pg_temp.reject(format('SELECT * FROM kehila_auth.reconcile_item_create(pg_temp.bad_authority(%L,%L::jsonb),''p'',repeat(''9'',32))',field,stale_value::text),'55000');
    PERFORM pg_temp.reject(format('SELECT * FROM kehila_auth.reconcile_item_edit(pg_temp.bad_authority(%L,%L::jsonb),''p'',''live-item'',repeat(''8'',32),1,convert_to(''{}'',''UTF8''))',field,stale_value::text),'55000');
  END LOOP;
END;
$$;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_create(pg_temp.bad_authority('owner_actor_id','"other"'),'p',repeat('9',32))$q$,'42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_edit(pg_temp.bad_authority('owner_actor_id','"other"'),'p','live-item',repeat('8',32),1,convert_to('{}','UTF8'))$q$,'42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_edit(pg_temp.authority(),'p','live-item','bad',1,''::bytea)$q$,'22023');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_edit(pg_temp.authority(),'p','live-item','bad',1,convert_to(repeat('x',1048577),'UTF8'))$q$,'22023');
SELECT pg_temp.require(NOT EXISTS(
  (SELECT request_id,xmin::text FROM kehila_auth.pending_items EXCEPT SELECT * FROM before_resolution)
  UNION ALL (SELECT * FROM before_resolution EXCEPT SELECT request_id,xmin::text FROM kehila_auth.pending_items)),
  'historical resolution leaves pending rows unchanged');
COMMIT;

UPDATE kehila.projects SET archived=true WHERE project_id='p';
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.require((SELECT outcome='replay' FROM kehila_auth.reconcile_item_create(
  pg_temp.authority(),'p',repeat('9',32))),'current archive does not hide recorded creation');
COMMIT;
UPDATE kehila.projects SET archived=false WHERE project_id='p';
UPDATE kehila.project_grants SET active=false WHERE project_id='p';
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_create(pg_temp.authority(),'p',repeat('9',32))$q$,'42501');
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_edit(pg_temp.authority(),'p','live-item',repeat('8',32),1,convert_to('{}','UTF8'))$q$,'42501');
COMMIT;
UPDATE kehila.project_grants SET active=true WHERE project_id='p';

-- Disposable owner corruption probes. Named protections are restored; these
-- qualify lookup classification, never resistance to administrative DDL.
CREATE TEMP TABLE original_payload AS SELECT p.* FROM kehila.operation_payloads p
  JOIN requests q ON q.operation_id=p.operation_row_id WHERE q.version=3;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila.operation_payloads DISABLE TRIGGER operation_payload_retirement_required;
ALTER TABLE kehila.operation_payloads DISABLE TRIGGER replay_payload_shape;
DELETE FROM kehila.operation_payloads WHERE operation_row_id IN(SELECT operation_row_id FROM original_payload);
ALTER TABLE kehila.operation_payloads ENABLE TRIGGER operation_payload_retirement_required;
ALTER TABLE kehila.operation_payloads ENABLE TRIGGER replay_payload_shape;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM requests q,LATERAL kehila_auth.reconcile_item_edit(
  pg_temp.authority(),'p','live-item',q.token::kehila.operation_token,1,q.bytes) WHERE q.version=3$q$,'23514');
COMMIT;
INSERT INTO kehila.operation_payloads SELECT * FROM original_payload;

BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila.operations DISABLE TRIGGER operation_core_immutable;
ALTER TABLE kehila.operations DISABLE TRIGGER operation_payload_shape;
UPDATE kehila.operations SET payload_retired=true WHERE operation_row_id=(SELECT operation_id FROM requests WHERE version=3);
ALTER TABLE kehila.operations ENABLE TRIGGER operation_core_immutable;
ALTER TABLE kehila.operations ENABLE TRIGGER operation_payload_shape;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM requests q,LATERAL kehila_auth.reconcile_item_edit(
  pg_temp.authority(),'p','live-item',q.token::kehila.operation_token,1,q.bytes) WHERE q.version=3$q$,'23514');
COMMIT;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila.operations DISABLE TRIGGER operation_core_immutable;
UPDATE kehila.operations SET payload_retired=false WHERE operation_row_id=(SELECT operation_id FROM requests WHERE version=3);
ALTER TABLE kehila.operations ENABLE TRIGGER operation_core_immutable;
COMMIT;

BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DROP CONSTRAINT pending_item_state_shape;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=NULL WHERE target_item_id='read-expired';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_create(pg_temp.authority(),'p',repeat('e',32))$q$,'23514');
COMMIT;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila_auth.pending_items DISABLE TRIGGER pending_item_transition;
UPDATE kehila_auth.pending_items SET request_bytes=convert_to('{}','UTF8') WHERE target_item_id='read-expired';
ALTER TABLE kehila_auth.pending_items ENABLE TRIGGER pending_item_transition;
ALTER TABLE kehila_auth.pending_items ADD CONSTRAINT pending_item_state_shape CHECK(
  (state='prepared' AND request_bytes IS NOT NULL AND operation_row_id IS NULL)
  OR (state IN ('cancelled','expired') AND request_bytes IS NULL AND operation_row_id IS NULL)
  OR (state='completed' AND operation_row_id IS NOT NULL));
COMMIT;

CREATE TEMP TABLE original_origin AS SELECT o.operation_row_id,o.replay_origin_ms,o.replay_deadline_ms
  FROM kehila.operations o JOIN requests q ON q.operation_id=o.operation_row_id WHERE q.version=2;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila.operations DISABLE TRIGGER operation_core_immutable;
DO $$ DECLARE future_origin kehila.u64:=kehila_auth.trusted_millis()+86400000;
BEGIN
  UPDATE kehila.operations SET replay_origin_ms=future_origin,replay_deadline_ms=future_origin+7776000000
    WHERE operation_row_id=(SELECT operation_row_id FROM original_origin);
END; $$;
ALTER TABLE kehila.operations ENABLE TRIGGER operation_core_immutable;
COMMIT;
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject($q$SELECT * FROM requests q,LATERAL kehila_auth.reconcile_item_edit(
  pg_temp.authority(),'p','live-item',q.token::kehila.operation_token,1,q.bytes) WHERE q.version=2$q$,'55000');
COMMIT;
BEGIN;
SET CONSTRAINTS ALL IMMEDIATE;
ALTER TABLE kehila.operations DISABLE TRIGGER operation_core_immutable;
UPDATE kehila.operations o SET replay_origin_ms=s.replay_origin_ms,replay_deadline_ms=s.replay_deadline_ms
  FROM original_origin s WHERE o.operation_row_id=s.operation_row_id;
ALTER TABLE kehila.operations ENABLE TRIGGER operation_core_immutable;
COMMIT;
SELECT pg_temp.reject($q$SELECT * FROM kehila_auth.reconcile_item_create(pg_temp.authority(),'p',repeat('9',32))$q$,'25000');
RESET ROLE;
SELECT 'PASS: scoped historical item reconciliation without execution' AS result;
