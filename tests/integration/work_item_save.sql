-- Fixed-entry owner controls. Typed Rust decisions/commit/release and concurrent
-- waits remain separate gates; this fixture prepares relationally valid packets.
\set ON_ERROR_STOP on
SET statement_timeout='15s';
SET ROLE kehila_owner;
CREATE FUNCTION pg_temp.require(ok boolean,label text) RETURNS void LANGUAGE plpgsql AS $$
BEGIN IF ok IS DISTINCT FROM true THEN RAISE EXCEPTION 'save: %',label; END IF; END;
$$;
CREATE FUNCTION pg_temp.reject(statement text,expected_state text,expected_constraint text DEFAULT '')
RETURNS void LANGUAGE plpgsql AS $$
DECLARE actual text; named text;
BEGIN
  BEGIN EXECUTE statement;
  EXCEPTION WHEN OTHERS THEN
    GET STACKED DIAGNOSTICS actual=RETURNED_SQLSTATE,named=CONSTRAINT_NAME;
    IF actual<>expected_state OR named<>expected_constraint THEN
      RAISE EXCEPTION 'wrong save rejection % / %, expected % / %',actual,named,expected_state,expected_constraint;
    END IF;
    RETURN;
  END;
  RAISE EXCEPTION 'expected save rejection absent: % / %',expected_state,expected_constraint;
END;
$$;
CREATE FUNCTION pg_temp.authority() RETURNS kehila_auth.authority LANGUAGE sql STABLE AS $$
  SELECT ROW(i.installation_id,i.owner_actor_id,i.runtime_generation,c.auth_generation,c.verifier_revision)::kehila_auth.authority
    FROM kehila_auth.installation i JOIN kehila_auth.owner_credentials c USING(owner_actor_id);
$$;
CREATE FUNCTION pg_temp.preparation(target text DEFAULT NULL) RETURNS jsonb LANGUAGE plpgsql STABLE AS $$
DECLARE p record; current_item record; packet jsonb;
BEGIN
  SELECT project.*,h.state_version,h.operation_row_id AS state_operation,c.operation_row_id AS configuration_operation,
      c.configuration_snapshot,c.project_snapshot AS configuration_project_snapshot
    INTO STRICT p FROM kehila.projects project JOIN kehila.project_operational_heads h USING(project_id)
    JOIN kehila.configuration_revisions c ON c.project_id=project.project_id AND c.revision=project.configuration_revision
    WHERE project.project_id='p';
  packet:=jsonb_build_object('project_snapshot',jsonb_build_object('codec_version',1,'kind','project_snapshot',
      'value',jsonb_build_object('id',p.project_id,'name',p.name,'prefix',p.current_prefix,'estimate_unit',p.estimate_unit,
        'configuration_revision',p.configuration_revision::numeric(20,0)::text,'next_sequence',p.next_sequence::numeric(20,0)::text,
        'ever_estimated',p.ever_estimated,'archived',p.archived)),
    'configuration_revision',p.configuration_revision,'configuration_operation',p.configuration_operation,
    'configuration_snapshot',p.configuration_snapshot,'configuration_project_snapshot',p.configuration_project_snapshot,
    'state_version',p.state_version,'state_operation',p.state_operation,
    'item_version',NULL,'item_operation',NULL,'item_history',NULL);
  IF target IS NOT NULL THEN
    SELECT i.current_version,r.operation_row_id,r.history INTO STRICT current_item
      FROM kehila.items i JOIN kehila.item_revisions r ON r.project_id=i.project_id
        AND r.item_id=i.item_id AND r.item_version=i.current_version WHERE i.project_id='p' AND i.item_id=target;
    packet:=packet||jsonb_build_object('item_version',current_item.current_version,
      'item_operation',current_item.operation_row_id,'item_history',current_item.history);
  END IF;
  RETURN packet;
END;
$$;
CREATE FUNCTION pg_temp.accepted_history(packet jsonb,creating boolean) RETURNS jsonb LANGUAGE plpgsql AS $$
DECLARE p jsonb:=packet #>'{project_snapshot,value}'; item_version text; saved jsonb;
BEGIN
  item_version:=CASE WHEN creating THEN '1' ELSE ((packet->>'item_version')::numeric+1)::numeric(20,0)::text END;
  IF creating THEN p:=jsonb_set(p,'{next_sequence}',to_jsonb(((p->>'next_sequence')::numeric+1)::numeric(20,0)::text)); END IF;
  p:=jsonb_set(p,'{ever_estimated}','true');
  saved:=jsonb_build_object('codec_version',1,'kind',CASE WHEN creating THEN 'item_create_result' ELSE 'item_edit_result' END,
    'value',jsonb_build_object('project',p,'item',jsonb_build_object('id','saved-item','project_id','p','type_id','milestone',
      'workflow_id','wf','status_id','new','version',item_version,'archived',false),
      'data',jsonb_build_object('estimate',CASE WHEN creating THEN '"0"'::jsonb ELSE 'null'::jsonb END,
        'values',CASE WHEN creating THEN '[{"id":"other-title","value":{"kind":"text","value":"new"}}]'::jsonb ELSE '[]'::jsonb END),
      'phase','new'));
  IF creating THEN
    saved:=jsonb_set(saved,'{value}',(saved->'value')||jsonb_build_object('readable_id',
      jsonb_build_object('project_id','p','item_id','saved-item','prefix',p->>'prefix',
        'sequence',packet #>>'{project_snapshot,value,next_sequence}')));
  END IF;
  RETURN jsonb_build_object('codec_version',1,'kind','item_revision','value',jsonb_build_object('actor_id','owner',
    'configuration_revision',(packet->>'configuration_revision')::numeric(20,0)::text,
    'project_state_version',((packet->>'state_version')::numeric+1)::numeric(20,0)::text,
    'previous_project_operation',packet->>'state_operation','result',saved));
END;
$$;
CREATE TEMP TABLE candidate AS SELECT
  convert_to('{"codec_version":1,"kind":"item_create_request","value":{"estimate":"0","expected_configuration_revision":"1","item_id":"saved-item","operation_id":"66666666666666666666666666666666","project_id":"p","type_id":"milestone","values":[{"id":"other-title","value":{"kind":"text","value":"new"}}],"workflow_id":null}}','UTF8') AS create_bytes,
  convert_to('{"codec_version":1,"kind":"item_edit_request","value":{"estimate":{"action":"clear"},"expected_configuration_revision":"1","expected_item_version":"1","fields":[{"edit":{"action":"clear"},"id":"other-title"}],"item_id":"saved-item","operation_id":"55555555555555555555555555555555","project_id":"p"}}','UTF8') AS edit_bytes;
SELECT * FROM kehila_auth.register_item(pg_temp.authority(),'p',repeat('6',32),repeat('6',32),'saved-item',1,
  (SELECT create_bytes FROM candidate));
CREATE FUNCTION pg_temp.save_create(packet jsonb,history jsonb,echo bytea DEFAULT NULL,locator text DEFAULT repeat('6',32),auth_override jsonb DEFAULT NULL)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE outcome text; expected kehila_auth.authority:=pg_temp.authority();
BEGIN
  IF auth_override IS NOT NULL THEN expected:=jsonb_populate_record(expected,auth_override); END IF;
  SELECT r.outcome INTO STRICT outcome FROM kehila_auth.save_item_create(expected,'p',locator::kehila_auth.intent_key,
    coalesce(echo,(SELECT create_bytes FROM candidate)),
    jsonb_populate_record(NULL::kehila_auth.item_preparation,packet),history) r;
  RETURN outcome;
END;
$$;
CREATE FUNCTION pg_temp.save_edit(packet jsonb,history jsonb,echo bytea DEFAULT NULL,token text DEFAULT repeat('5',32),auth_override jsonb DEFAULT NULL)
RETURNS text LANGUAGE plpgsql AS $$
DECLARE outcome text; expected kehila_auth.authority:=pg_temp.authority();
BEGIN
  IF auth_override IS NOT NULL THEN expected:=jsonb_populate_record(expected,auth_override); END IF;
  SELECT r.outcome INTO STRICT outcome FROM kehila_auth.save_item_edit(expected,'p','saved-item',token::kehila.operation_token,
    coalesce(echo,(SELECT edit_bytes FROM candidate)),
    jsonb_populate_record(NULL::kehila_auth.item_preparation,packet),history) r;
  RETURN outcome;
END;
$$;

-- First call intentionally reaches the missing preparation type in the red
-- candidate. Typed argument construction precedes function-body lookup.
BEGIN;
SELECT pg_temp.require(pg_temp.save_create(pg_temp.preparation(),pg_temp.accepted_history(pg_temp.preparation(),true))='accepted',
  'fresh create accepts the single admitted history');
SET CONSTRAINTS ALL IMMEDIATE;
ROLLBACK;

DO $$ DECLARE p jsonb:=pg_temp.preparation(); h jsonb; BEGIN
  h:=pg_temp.accepted_history(p,true);
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(p,'{configuration_revision}','2'),h)='configuration_conflict','configuration mismatch');
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(jsonb_set(p,'{configuration_revision}','2'),'{state_version}',to_jsonb((p->>'state_version')::numeric+1)),h)='configuration_conflict','configuration conflict precedes head re-preparation');
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(p,'{configuration_snapshot}','{"changed":true}'),h)='configuration_conflict','immutable configuration mismatch');
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(p,'{project_snapshot,value,name}','"changed"'),h)='configuration_conflict','stable metadata mismatch');
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(p,'{state_version}',to_jsonb((p->>'state_version')::numeric+1)),h)='reprepare','head version requires rollback/reprepare');
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(p,'{state_operation}',to_jsonb((p->>'state_operation')::bigint+1)),h)='reprepare','unchanged-counter predecessor mismatch');
  PERFORM pg_temp.require(pg_temp.save_create(jsonb_set(p,'{project_snapshot,value,ever_estimated}','false'),h)='reprepare','estimate witness cannot be overwritten');
  PERFORM pg_temp.require(pg_temp.save_create(NULL,NULL,(SELECT create_bytes||convert_to(' ','UTF8') FROM candidate))='conflict','prepared echo conflict precedes fresh parameters');
  PERFORM pg_temp.reject('SELECT pg_temp.save_create(NULL,NULL)','22023');
  PERFORM pg_temp.reject(format('SELECT pg_temp.save_create(%L::jsonb,%L::jsonb,auth_override=>%L::jsonb)',p::text,h::text,
    jsonb_build_object('runtime_generation',(pg_temp.authority()).runtime_generation+1)::text),'55000');
  PERFORM pg_temp.reject(format('UPDATE kehila.project_grants SET active=false WHERE project_id=''p''; SELECT pg_temp.save_create(%L::jsonb,%L::jsonb)',p::text,h::text),'42501');
END; $$;

CREATE FUNCTION kehila_auth.test_save_fault() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'injected save boundary: %',TG_TABLE_NAME USING ERRCODE='P0001',CONSTRAINT='save_fault'; END;
$$;
DO $$ DECLARE boundary text; relation text; event text; p jsonb:=pg_temp.preparation(); h jsonb;
BEGIN
  h:=pg_temp.accepted_history(p,true);
  FOREACH boundary IN ARRAY ARRAY['kehila.operations','kehila.operation_payloads','kehila.items','kehila.item_revisions',
    'kehila.item_field_uses','kehila.projects','kehila.project_operational_heads','kehila_auth.pending_items'] LOOP
    relation:=boundary;
    event:=CASE WHEN boundary IN ('kehila.projects','kehila.project_operational_heads','kehila_auth.pending_items') THEN 'UPDATE' ELSE 'INSERT' END;
    EXECUTE format('CREATE TRIGGER save_fault AFTER %s ON %s FOR EACH ROW EXECUTE FUNCTION kehila_auth.test_save_fault()',event,relation);
    PERFORM pg_temp.reject(format('SELECT pg_temp.save_create(%L::jsonb,%L::jsonb)',p::text,h::text),'P0001','save_fault');
    EXECUTE format('DROP TRIGGER save_fault ON %s',relation);
    PERFORM pg_temp.require((SELECT count(*)=9 FROM kehila.operations)
      AND (SELECT count(*)=2 FROM kehila.items) AND (SELECT count(*)=7 FROM kehila.item_revisions)
      AND (SELECT state='prepared' AND operation_row_id IS NULL FROM kehila_auth.pending_items WHERE target_item_id='saved-item'),
      'failed write boundary rolls back every accepted record');
  END LOOP;
END; $$;
BEGIN;
SELECT pg_temp.require(pg_temp.save_create(pg_temp.preparation(),pg_temp.accepted_history(pg_temp.preparation(),true))='accepted','committed Milestone create');
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require(pg_temp.save_create(NULL,NULL)='replay','recorded create ignores absent fresh packet');
SELECT pg_temp.require(pg_temp.save_create(NULL,NULL,(SELECT create_bytes||convert_to(' ','UTF8') FROM candidate))='conflict','changed completed echo is request conflict');

CREATE TEMP TABLE edit_prepared AS SELECT pg_temp.preparation('saved-item') AS packet;
DO $$ DECLARE p jsonb:=(SELECT packet FROM edit_prepared); h jsonb;
BEGIN
  h:=pg_temp.accepted_history(p,false);
  PERFORM pg_temp.reject(format('SELECT pg_temp.save_edit(%L::jsonb,%L::jsonb)',p::text,
    jsonb_set(h,'{value,previous_project_operation}',to_jsonb(p->>'configuration_operation'))::text),
    '23514','item_history_agreement');
  PERFORM pg_temp.require(pg_temp.save_edit(jsonb_set(p,'{item_version}','2'),h)='item_conflict','item version mismatch');
  PERFORM pg_temp.require(pg_temp.save_edit(jsonb_set(jsonb_set(p,'{item_version}','2'),'{state_version}',to_jsonb((p->>'state_version')::numeric+1)),h)='item_conflict','item conflict precedes head re-preparation');
  PERFORM pg_temp.require(pg_temp.save_edit(jsonb_set(p,'{item_operation}',to_jsonb((p->>'item_operation')::bigint+1)),h)='item_conflict','immutable item revision mismatch');
  PERFORM pg_temp.require(pg_temp.save_edit(jsonb_set(p,'{item_history}','{}'),h)='item_conflict','complete item content mismatch');
  PERFORM pg_temp.require(pg_temp.save_edit(jsonb_set(p,'{state_version}',to_jsonb((p->>'state_version')::numeric+1)),h)='reprepare','head-only edit mismatch');
  PERFORM pg_temp.reject(format('SELECT pg_temp.save_edit(%L::jsonb,%L::jsonb,auth_override=>%L::jsonb)',p::text,h::text,
    jsonb_build_object('runtime_generation',(pg_temp.authority()).runtime_generation+1)::text),'55000');
  PERFORM pg_temp.reject(format('UPDATE kehila.project_grants SET active=false WHERE project_id=''p''; SELECT pg_temp.save_edit(%L::jsonb,%L::jsonb)',p::text,h::text),'42501');
END; $$;
CREATE TRIGGER save_fault AFTER UPDATE ON kehila.items FOR EACH ROW EXECUTE FUNCTION kehila_auth.test_save_fault();
SELECT pg_temp.reject(format('SELECT pg_temp.save_edit(%L::jsonb,%L::jsonb)',packet::text,
  pg_temp.accepted_history(packet,false)::text),'P0001','save_fault') FROM edit_prepared;
DROP TRIGGER save_fault ON kehila.items;
DROP FUNCTION kehila_auth.test_save_fault();
SELECT pg_temp.require((SELECT count(*)=10 FROM kehila.operations)
  AND (SELECT count(*)=8 FROM kehila.item_revisions)
  AND (SELECT current_version=1 FROM kehila.items WHERE item_id='saved-item'),
  'edit current-pointer failure rolls back its core, payload and revision');
BEGIN;
SELECT pg_temp.require(pg_temp.save_edit(packet,pg_temp.accepted_history(packet,false))='accepted','committed sparse clear result') FROM edit_prepared;
SET CONSTRAINTS ALL IMMEDIATE;
COMMIT;
SELECT pg_temp.require(pg_temp.save_edit(NULL,NULL)='replay','recorded edit ignores absent fresh packet');
SELECT pg_temp.require(pg_temp.save_edit(NULL,NULL,(SELECT edit_bytes||convert_to(' ','UTF8') FROM candidate))='conflict','changed edit conflicts');
SELECT pg_temp.require(pg_temp.save_edit(packet,pg_temp.accepted_history(packet,false),
  (SELECT convert_to(replace(convert_from(edit_bytes,'UTF8'),repeat('5',32),repeat('4',32)),'UTF8') FROM candidate),repeat('4',32))='item_conflict',
  'two edits at one version conflict before head re-preparation') FROM edit_prepared;
DO $$ DECLARE stale jsonb:=jsonb_build_object('runtime_generation',(pg_temp.authority()).runtime_generation+1);
BEGIN
  PERFORM pg_temp.reject(format('SELECT pg_temp.save_create(NULL,NULL,auth_override=>%L::jsonb)',stale::text),'55000');
  PERFORM pg_temp.reject(format('SELECT pg_temp.save_edit(NULL,NULL,auth_override=>%L::jsonb)',stale::text),'55000');
  PERFORM pg_temp.reject('UPDATE kehila.project_grants SET active=false WHERE project_id=''p''; SELECT pg_temp.save_create(NULL,NULL)','42501');
  PERFORM pg_temp.reject('UPDATE kehila.project_grants SET active=false WHERE project_id=''p''; SELECT pg_temp.save_edit(NULL,NULL)','42501');
END; $$;
UPDATE kehila.projects SET archived=true WHERE project_id='p';
SELECT pg_temp.require(pg_temp.save_create(NULL,NULL)='replay' AND pg_temp.save_edit(NULL,NULL)='replay',
  'current archive cannot mask recorded save success');
UPDATE kehila.projects SET archived=false WHERE project_id='p';
BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY;
SELECT pg_temp.reject('SELECT pg_temp.save_create(NULL,NULL)','25000');
SELECT pg_temp.reject('SELECT pg_temp.save_edit(NULL,NULL)','25000');
ROLLBACK;

-- RC isolation alone is insufficient when the transaction is read-only.
BEGIN READ ONLY;
SELECT pg_temp.reject('SELECT pg_temp.save_create(NULL,NULL)','25000');
SELECT pg_temp.reject('SELECT pg_temp.save_edit(NULL,NULL)','25000');
ROLLBACK;

-- Constructed expiry checks the final guard classification, not an observed
-- wait/deadline crossing. Other deferred attribution has not yet been checked.
BEGIN;
INSERT INTO kehila_auth.pending_items(installation_id,actor_id,project_id,intent_handle,token,target_item_id,
  request_codec_version,request_sha256,request_bytes,registered_ms,expires_ms)
VALUES('installation','owner','p',repeat('4',32),repeat('4',32),'expired-save',1,sha256(convert_to('{}','UTF8')),convert_to('{}','UTF8'),0,60000);
SELECT pg_temp.require(pg_temp.save_create(NULL,NULL,convert_to('{}','UTF8'),repeat('4',32))='expired','expired unseen intent never executes');
SELECT pg_temp.reject($q$UPDATE kehila_auth.pending_items SET state='completed',operation_row_id=(SELECT creation_operation_row_id
  FROM kehila.items WHERE item_id='saved-item') WHERE target_item_id='expired-save'$q$,'P0002','pending_item_expired');
ROLLBACK;
-- A fresh unseen edit for the separate native predecessor-wait control. The
-- control rolls it back and removes this fixture-only packet before observing.
CREATE TABLE kehila_auth.test_wait_packet AS
  SELECT pg_temp.authority() AS authority,
    jsonb_populate_record(NULL::kehila_auth.item_preparation,p.packet) AS prepared,
    pg_temp.accepted_history(p.packet,false) AS history,
    convert_to(replace(replace(convert_from(c.edit_bytes,'UTF8'),repeat('5',32),'wait-core'),
      '"expected_item_version":"1"','"expected_item_version":"2"'),'UTF8') AS request_bytes
  FROM (SELECT pg_temp.preparation('saved-item') AS packet)p CROSS JOIN candidate c;
RESET ROLE;
SELECT 'PASS: protected atomic item saves, conflict classes and rollback boundaries' AS result;
